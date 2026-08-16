use std::collections::BTreeSet;

use lopdf::{Dictionary, Document, Object, ObjectId};

use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::object_resolution::resolve_optional;

/// A reached `/Type /Page` leaf, identified either by its indirect object id
/// or, when the page dictionary is embedded directly in a `Kids` array
/// instead of referenced indirectly, by a clone of the dictionary itself.
///
/// PDF32000 requires page-tree `Kids` entries to be indirect references, but
/// veraPDF 1.30.2 does not enforce this: confirmed by embedding a
/// compliant document's sole page directly and observing the same
/// annotation-subtype and flag violations reported as when it is indirect.
/// Every page-tree consumer resolves a page through [`PageEntry::resolve`]
/// instead of assuming `document.objects.get(&page_id)`.
#[derive(Clone, Debug)]
pub(crate) enum PageEntry {
    Indirect(ObjectId),
    Direct(Dictionary),
}

impl PageEntry {
    /// Returns the page dictionary, re-resolving an indirect entry through
    /// `document` (which can only fail if `document` differs from the one
    /// `collect_pages` walked) or borrowing a direct entry's own clone.
    pub(crate) fn resolve<'a>(&'a self, document: &'a Document) -> Option<&'a Dictionary> {
        match self {
            Self::Indirect(id) => document
                .objects
                .get(id)
                .and_then(|object| object.as_dict().ok()),
            Self::Direct(dictionary) => Some(dictionary),
        }
    }

    /// The page's indirect object id, or `None` for a directly embedded
    /// page dictionary with no id of its own.
    pub(crate) fn object_id(&self) -> Option<ObjectId> {
        match self {
            Self::Indirect(id) => Some(*id),
            Self::Direct(_) => None,
        }
    }
}

/// Walks the catalog `/Pages` tree, collecting each reached `/Type /Page`
/// leaf, numbered in visitation order starting at 1.
///
/// This is the single traversal every page-tree consumer (page counting,
/// content execution, annotations, actions, forms, font embedding, ICCBased
/// and graphics inspection) shares, replacing both `lopdf::Document::
/// get_pages` (which bounds itself with a hardcoded depth and a
/// document-object-count iteration budget instead of this crate's
/// `SafetyLimits`, and never surfaces a `PdfError`) and a previous
/// hand-rolled `count_pages` that silently under-counted certain cycles
/// instead of reporting them.
///
/// Bounded exactly like every other traversal in this crate: depth is capped
/// by `limits.max_reference_depth`, and a *true* cycle (an indirect object
/// id revisited while still an ancestor on the current path) raises
/// `PdfError::ReferenceDepth` rather than silently truncating the page
/// list. Cycle detection is ancestor-path-scoped, not "ever visited
/// anywhere": confirmed against veraPDF 1.30.2 that the *same* Page object
/// legitimately reachable through two different `Pages` branches (a DAG,
/// not a cycle — neither branch is the other's ancestor) is processed
/// without error, so an earlier global "ever visited" set was a false
/// positive relative to veraPDF and has been replaced. A separate step
/// counter, bounded by `limits.max_object_count`, still caps the total
/// walk — without it, a pathological DAG that doesn't repeat any single
/// object as its own ancestor (so ancestor-scoped tracking alone would
/// never reject it) could still make the walk's total work exponential in
/// its depth by fanning out shared subtrees.
///
/// A `Kids` entry is followed whether it is an indirect reference or a
/// direct dictionary (see [`PageEntry`]); an unresolvable reference or a
/// resolved non-dictionary target is silently skipped.
///
/// A *resolved* page-tree node (root or `Kids` entry) whose `/Type` is
/// missing or is neither `Page` nor `Pages` is a parse-level failure
/// (`PdfError::UnexpectedObject`, surfacing as `PDF-PARSE-001`), not a
/// silently skipped node: confirmed against veraPDF 1.30.2, which throws a
/// fatal `unknown type of page tree node` parse exception — rejecting the
/// whole file before any conformance rule runs — for both a missing and a
/// present-but-wrong page-tree node `/Type`.
pub(crate) fn collect_pages(
    document: &Document,
    catalog: &Dictionary,
    limits: &SafetyLimits,
) -> Result<Vec<PageEntry>, PdfError> {
    let mut pages = Vec::new();
    let Ok(root) = catalog.get(b"Pages") else {
        return Ok(pages);
    };
    let mut ancestors = BTreeSet::new();
    let mut steps = 0usize;
    walk(
        document,
        root,
        limits,
        0,
        &mut ancestors,
        &mut steps,
        &mut pages,
    )?;
    Ok(pages)
}

/// Walks one node, tracking `ancestors` (ids currently on the path from the
/// root to this call, for true-cycle detection) and `steps` (a running
/// total of every node visited anywhere in the walk, for DAG-blowup
/// safety). `ancestors` is restored to its pre-call state before returning
/// on every path, success or failure, so a sibling branch that legitimately
/// shares a node with an already-completed branch is not mistaken for that
/// node being its own ancestor.
fn walk(
    document: &Document,
    node: &Object,
    limits: &SafetyLimits,
    depth: usize,
    ancestors: &mut BTreeSet<ObjectId>,
    steps: &mut usize,
    pages: &mut Vec<PageEntry>,
) -> Result<(), PdfError> {
    if depth >= limits.max_reference_depth {
        return Err(PdfError::ReferenceDepth(limits.max_reference_depth));
    }
    *steps += 1;
    if *steps > limits.max_object_count {
        return Err(PdfError::TooManyObjects {
            actual: *steps,
            limit: limits.max_object_count,
        });
    }
    let object_id = node.as_reference().ok();
    if let Some(id) = object_id
        && !ancestors.insert(id)
    {
        return Err(PdfError::ReferenceDepth(limits.max_reference_depth));
    }
    let result = walk_node(
        document, node, object_id, limits, depth, ancestors, steps, pages,
    );
    if let Some(id) = object_id {
        ancestors.remove(&id);
    }
    result
}

fn walk_node(
    document: &Document,
    node: &Object,
    object_id: Option<ObjectId>,
    limits: &SafetyLimits,
    depth: usize,
    ancestors: &mut BTreeSet<ObjectId>,
    steps: &mut usize,
    pages: &mut Vec<PageEntry>,
) -> Result<(), PdfError> {
    let resolved = match object_id {
        Some(_) => {
            let Some(resolved) = resolve_optional(document, node, limits.max_reference_depth)?
            else {
                return Ok(());
            };
            resolved
        }
        None => node,
    };
    let Ok(dictionary) = resolved.as_dict() else {
        return Ok(());
    };
    match dictionary.get_type().ok() {
        Some(b"Page") => {
            let entry = match object_id {
                Some(id) => PageEntry::Indirect(id),
                None => PageEntry::Direct(dictionary.clone()),
            };
            pages.push(entry);
        }
        Some(b"Pages") => {
            let Ok(kids) = dictionary.get(b"Kids") else {
                return Ok(());
            };
            let Some(kids) = resolve_optional(document, kids, limits.max_reference_depth)?
                .and_then(|object| object.as_array().ok())
            else {
                return Ok(());
            };
            for kid in kids {
                walk(document, kid, limits, depth + 1, ancestors, steps, pages)?;
            }
        }
        _ => {
            return Err(PdfError::UnexpectedObject(
                "page tree node has a missing or unrecognized /Type",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use lopdf::{Dictionary, Document, Object, dictionary};

    use super::collect_pages;
    use crate::SafetyLimits;

    fn catalog_pages(document: &Document) -> &Dictionary {
        document
            .trailer
            .get(b"Root")
            .ok()
            .and_then(|root| root.as_reference().ok())
            .and_then(|id| document.objects.get(&id))
            .and_then(|object| object.as_dict().ok())
            .expect("catalog dictionary")
    }

    #[test]
    fn two_level_page_tree_cycle_is_a_reference_depth_error() {
        let mut document = Document::with_version("1.4");
        let a_id = document.new_object_id();
        let b_id = document.new_object_id();
        document.objects.insert(
            a_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(b_id)],
                "Count" => 1,
            }),
        );
        document.objects.insert(
            b_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(a_id)],
                "Count" => 1,
            }),
        );
        let catalog_id = document.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => a_id,
        });
        document.trailer.set("Root", Object::Reference(catalog_id));

        let catalog = catalog_pages(&document);
        let result = collect_pages(&document, catalog, &SafetyLimits::default());
        assert!(matches!(result, Err(crate::PdfError::ReferenceDepth(_))));
    }

    /// Confirmed against veraPDF 1.30.2: a page-tree node reached via `Kids`
    /// with a missing or unrecognized `/Type` throws a fatal
    /// `unknown type of page tree node` parse exception, rather than being
    /// silently skipped.
    #[test]
    fn rejects_kids_entries_with_a_missing_or_wrong_type() {
        for untyped in [dictionary! {}, dictionary! { "Type" => "Foo" }] {
            let mut document = Document::with_version("1.4");
            let page_id = document.add_object(dictionary! {
                "Type" => "Page",
            });
            let untyped_id = document.add_object(untyped);
            let pages_id = document.add_object(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id), Object::Reference(untyped_id)],
                "Count" => 2,
            });
            let catalog_id = document.add_object(dictionary! {
                "Type" => "Catalog",
                "Pages" => pages_id,
            });
            document.trailer.set("Root", Object::Reference(catalog_id));

            let catalog = catalog_pages(&document);
            let result = collect_pages(&document, catalog, &SafetyLimits::default());
            assert!(matches!(result, Err(crate::PdfError::UnexpectedObject(_))));
        }
    }

    /// Confirmed against veraPDF 1.30.2: the *same* Page object reachable
    /// through two different `Pages` branches (a DAG, not a cycle — neither
    /// branch is the other's ancestor) is processed without error.
    #[test]
    fn shared_page_reached_from_two_branches_is_not_a_cycle() {
        let mut document = Document::with_version("1.4");
        let page_id = document.add_object(dictionary! { "Type" => "Page" });
        let branch_a = document.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1,
        });
        let branch_b = document.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1,
        });
        let root_pages = document.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(branch_a), Object::Reference(branch_b)],
            "Count" => 2,
        });
        let catalog_id = document.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => root_pages,
        });
        document.trailer.set("Root", Object::Reference(catalog_id));

        let catalog = catalog_pages(&document);
        let pages =
            collect_pages(&document, catalog, &SafetyLimits::default()).expect("collect pages");
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].object_id(), Some(page_id));
        assert_eq!(pages[1].object_id(), Some(page_id));
    }

    /// Confirmed against veraPDF 1.30.2: a Page dictionary embedded directly
    /// (not as an indirect reference) in a `Kids` array is still walked and
    /// validated, not silently ignored.
    #[test]
    fn collects_a_directly_embedded_page() {
        let mut document = Document::with_version("1.4");
        let indirect_page_id = document.add_object(dictionary! { "Type" => "Page" });
        let direct_page = Object::Dictionary(dictionary! { "Type" => "Page" });
        let pages_id = document.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(indirect_page_id), direct_page],
            "Count" => 2,
        });
        let catalog_id = document.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        document.trailer.set("Root", Object::Reference(catalog_id));

        let catalog = catalog_pages(&document);
        let pages =
            collect_pages(&document, catalog, &SafetyLimits::default()).expect("collect pages");
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].object_id(), Some(indirect_page_id));
        assert_eq!(pages[1].object_id(), None);
        assert!(pages[1].resolve(&document).is_some());
    }
}
