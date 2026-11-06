//! Maps indirect PDF objects to the pages that own or use them for report locations.
//!
//! The shared page list supplies page-owned objects, and content execution supplies used resources. Cached dependency lists follow their indirect children without decoding streams or following resource inventories, page-tree parents, or navigation links. Per-page visited sets stop cycles, and depth bounds limit diagnostic traversal without introducing new validation failures.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use lopdf::{Dictionary, Document, Object, ObjectId};

use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::page_tree::PageEntry;

/// Collects indirect dependencies embedded in a value, without resolving references.
pub(crate) fn references(object: &Object, ids: &mut BTreeSet<ObjectId>, depth: usize) {
    match object {
        Object::Reference(id) => {
            ids.insert(*id);
        }
        _ if depth == 0 => {}
        Object::Array(items) => {
            for item in items {
                references(item, ids, depth - 1);
            }
        }
        Object::Dictionary(dictionary) => dictionary_references(dictionary, ids, depth - 1),
        Object::Stream(stream) => dictionary_references(&stream.dict, ids, depth - 1),
        _ => {}
    }
}

fn dictionary_references(dictionary: &Dictionary, ids: &mut BTreeSet<ObjectId>, depth: usize) {
    let destination_action = dictionary
        .get(b"S")
        .and_then(Object::as_name)
        .is_ok_and(|name| matches!(name, b"GoTo" | b"GoToR" | b"GoToE" | b"Thread"));
    for (key, value) in dictionary.iter() {
        if matches!(
            key.as_slice(),
            b"Parent" | b"P" | b"Pg" | b"Pages" | b"StructTreeRoot" | b"Resources" | b"Dest"
        ) || (destination_action && key == b"D")
        {
            continue;
        }
        references(value, ids, depth);
    }
}

/// Expands page ownership and observed resource use into sorted, unique page positions.
pub(crate) fn collect(
    document: &Document,
    pages: &[PageEntry],
    used: &BTreeMap<u32, BTreeSet<ObjectId>>,
    limits: &SafetyLimits,
) -> BTreeMap<PdfObjectId, Vec<usize>> {
    let mut locations = BTreeMap::<PdfObjectId, Vec<usize>>::new();
    let mut dependencies = BTreeMap::<ObjectId, BTreeSet<ObjectId>>::new();
    for (index, page) in pages.iter().enumerate() {
        let page_number = index + 1;
        let mut roots = BTreeSet::new();
        if let Some(id) = page.object_id() {
            roots.insert(id);
        } else if let Some(dictionary) = page.resolve(document) {
            dictionary_references(dictionary, &mut roots, limits.max_reference_depth);
        }
        if let Ok(number) = u32::try_from(page_number)
            && let Some(used) = used.get(&number)
        {
            roots.extend(used);
        }
        let mut pending: VecDeque<_> = roots.into_iter().map(|id| (id, 0)).collect();
        let mut visited = BTreeSet::new();
        while let Some((id, depth)) = pending.pop_front() {
            if !visited.insert(id) {
                continue;
            }
            let Some(object) = document.objects.get(&id) else {
                continue;
            };
            if let Ok(dictionary) = object.as_dict() {
                let kind = dictionary.get(b"Type").and_then(Object::as_name).ok();
                if matches!(kind, Some(b"Catalog" | b"Pages"))
                    || (kind == Some(b"Page") && page.object_id() != Some(id))
                {
                    continue;
                }
            }
            locations.entry(id.into()).or_default().push(page_number);
            if depth >= limits.max_reference_depth {
                continue;
            }
            let children = dependencies.entry(id).or_insert_with(|| {
                let mut children = BTreeSet::new();
                references(object, &mut children, limits.max_reference_depth);
                children
            });
            pending.extend(children.iter().map(|child| (*child, depth + 1)));
        }
    }
    locations
}

#[cfg(test)]
mod tests {
    use lopdf::{Stream, dictionary};

    use super::*;
    use crate::content_support::{ContentCache, execute_content};

    #[test]
    fn maps_owned_objects_and_used_resources_without_unused_or_navigation_targets() {
        let mut document = Document::with_version("1.4");
        let pages_id = document.new_object_id();
        let program = document.add_object(Stream::new(Dictionary::new(), Vec::new()));
        let descriptor = document.add_object(dictionary! { "FontFile" => program });
        let shared_font = document.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "FontDescriptor" => descriptor });
        document
            .objects
            .get_mut(&program)
            .expect("program")
            .as_stream_mut()
            .expect("stream")
            .dict
            .set("Cycle", shared_font);
        let second_font = document.add_object(
            dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Courier" },
        );
        let form_font = document.add_object(
            dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Times-Roman" },
        );
        let unused_font = document.add_object(
            dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Times-Bold" },
        );
        let form = document.add_object(Stream::new(dictionary! {
            "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
            "Resources" => dictionary! { "Font" => dictionary! { "FF" => form_font, "Unused" => unused_font } },
        }, b"BT /FF 10 Tf (C) Tj ET".to_vec()));
        let shared_content = document.add_object(Stream::new(
            Dictionary::new(),
            b"BT /F1 10 Tf (A) Tj ET".to_vec(),
        ));
        let second_content = document.add_object(Stream::new(
            Dictionary::new(),
            b"BT /F2 10 Tf (B) Tj ET /Fm Do /Fm Do".to_vec(),
        ));
        let third_content = document.add_object(Stream::new(Dictionary::new(), b"/Fm Do".to_vec()));
        let third_page = document.add_object(
            dictionary! { "Type" => "Page", "Parent" => pages_id, "Contents" => third_content },
        );
        let appearance = document.add_object(Stream::new(dictionary! { "Subtype" => "Form", "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()] }, Vec::new()));
        let annotation = document.add_object(dictionary! {
            "Type" => "Annot", "Subtype" => "Link", "Rect" => vec![0.into(), 0.into(), 10.into(), 10.into()],
            "AP" => dictionary! { "N" => appearance, "D" => appearance },
            "A" => dictionary! { "S" => "GoTo", "D" => vec![Object::Reference(third_page), Object::Name(b"Fit".to_vec())] },
        });
        let first_page = document.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id, "Contents" => shared_content, "Annots" => vec![Object::Reference(annotation)] });
        let second_page = document.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id, "Contents" => vec![Object::Reference(shared_content), Object::Reference(second_content)], "Annots" => vec![Object::Reference(annotation)] });
        let blank_page =
            document.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id });
        document
            .objects
            .get_mut(&annotation)
            .expect("annotation")
            .as_dict_mut()
            .expect("dictionary")
            .set("P", first_page);
        document.set_object(pages_id, dictionary! { "Type" => "Pages", "Resources" => dictionary! {
            "Font" => dictionary! { "F1" => shared_font, "F2" => second_font, "Unused" => unused_font },
            "XObject" => dictionary! { "Fm" => form },
        }});
        let direct_page =
            dictionary! { "Type" => "Page", "Parent" => pages_id, "Contents" => shared_content };
        let pages = vec![
            PageEntry::Indirect(first_page),
            PageEntry::Indirect(second_page),
            PageEntry::Indirect(third_page),
            PageEntry::Indirect(blank_page),
            PageEntry::Direct(direct_page),
        ];
        let metadata = document.add_object(Stream::new(Dictionary::new(), Vec::new()));
        let catalog = document.add_object(
            dictionary! { "Type" => "Catalog", "Pages" => pages_id, "Metadata" => metadata },
        );
        let limits = SafetyLimits::default();
        let content = execute_content(
            &document,
            &pages,
            &mut ContentCache::new(),
            &BTreeSet::new(),
            &limits,
            true,
        )
        .expect("execute page content");
        let locations = collect(&document, &pages, &content.page_objects, &limits);
        for id in [shared_font, descriptor, program, shared_content] {
            assert_eq!(
                locations.get(&id.into()),
                Some(&vec![1, 2, 5]),
                "object {id:?}"
            );
        }
        assert_eq!(locations.get(&second_font.into()), Some(&vec![2]));
        for id in [form, form_font] {
            assert_eq!(
                locations.get(&id.into()),
                Some(&vec![2, 3]),
                "object {id:?}"
            );
        }
        for id in [annotation, appearance] {
            assert_eq!(
                locations.get(&id.into()),
                Some(&vec![1, 2]),
                "object {id:?}"
            );
        }
        assert_eq!(locations.get(&third_page.into()), Some(&vec![3]));
        for id in [unused_font, catalog, metadata, pages_id] {
            assert!(
                !locations.contains_key(&id.into()),
                "object {id:?} has no page owner or use"
            );
        }
        let without_locations = execute_content(
            &document,
            &pages,
            &mut ContentCache::new(),
            &BTreeSet::new(),
            &limits,
            false,
        )
        .expect("execute without report locations");
        assert!(without_locations.page_objects.is_empty());
    }

    #[test]
    fn dependency_cycles_and_reference_depth_are_bounded() {
        let mut document = Document::with_version("1.4");
        let first = document.new_object_id();
        let second = document.add_object(dictionary! { "Next" => first });
        document.set_object(first, dictionary! { "Next" => second });
        let page = document.add_object(dictionary! { "Type" => "Page", "Contents" => first });
        let pages = [PageEntry::Indirect(page)];
        let mut limits = SafetyLimits::default().with_max_reference_depth(1);
        let locations = collect(&document, &pages, &BTreeMap::new(), &limits);
        assert!(locations.contains_key(&first.into()));
        assert!(!locations.contains_key(&second.into()));
        limits = limits.with_max_reference_depth(8);
        let locations = collect(&document, &pages, &BTreeMap::new(), &limits);
        assert_eq!(locations.get(&first.into()), Some(&vec![1]));
        assert_eq!(locations.get(&second.into()), Some(&vec![1]));
    }
}
