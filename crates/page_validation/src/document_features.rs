use std::collections::{BTreeMap, BTreeSet};

use lopdf::{Document, Object, ObjectId};

use crate::catalog::{resolve_catalog, root_reference_id};
use crate::error::PdfError;
use crate::file_spec;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::object_resolution::{dictionary_based, resolve_optional};
use crate::report::RuleFailure;

#[derive(Clone, Debug, Default)]
pub(crate) struct DocumentFeatureSummary {
    pub(crate) catalog_id: Option<PdfObjectId>,
    pub(crate) mark_info_object_id: Option<PdfObjectId>,
    pub(crate) mark_info_is_dictionary: bool,
    pub(crate) marked: Option<bool>,
    pub(crate) contains_embedded_files_name: bool,
    pub(crate) contains_optional_content: bool,
    pub(crate) file_specs_with_embedded_files: Vec<RuleFailure>,
    pub(crate) struct_tree_root_object_id: Option<PdfObjectId>,
    pub(crate) struct_tree_root_present: bool,
    pub(crate) struct_tree_root_valid: bool,
    pub(crate) struct_tree_role_map_has_cycle: bool,
    pub(crate) struct_tree_has_unmapped_type: bool,
    pub(crate) language_failures: Vec<RuleFailure>,
}

pub(crate) fn inspect(
    document: &Document,
    limits: &SafetyLimits,
) -> Result<DocumentFeatureSummary, PdfError> {
    let catalog_id = root_reference_id(document);
    let Some(catalog) = resolve_catalog(document, limits)?.map(|catalog| catalog.dictionary) else {
        return Ok(DocumentFeatureSummary {
            catalog_id,
            ..DocumentFeatureSummary::default()
        });
    };

    let (mark_info_object_id, mark_info_is_dictionary, marked) = catalog
        .get(b"MarkInfo")
        .ok()
        .map(|value| -> Result<_, PdfError> {
            let object_id = value.as_reference().ok().map(Into::into);
            let resolved = resolve_optional(document, value, limits.max_reference_depth)?;
            let Some(dictionary) = resolved.and_then(|object| object.as_dict().ok()) else {
                return Ok((object_id, false, None));
            };
            let marked = dictionary
                .get(b"Marked")
                .ok()
                .map(|value| resolve_optional(document, value, limits.max_reference_depth))
                .transpose()?
                .flatten()
                .and_then(|object| object.as_bool().ok());
            Ok((object_id, true, marked))
        })
        .transpose()?
        .unwrap_or((None, false, None));

    let structure_tree = inspect_structure_tree(document, catalog, limits)?;
    let mut language_failures = Vec::new();
    if let Some(failure) = crate::language::inspect_dictionary(
        document,
        limits,
        catalog,
        catalog_id,
        "document catalog",
    ) {
        language_failures.push(failure);
    }

    let names = catalog
        .get(b"Names")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|object| match object {
            Object::Dictionary(dictionary) => Some(dictionary),
            _ => None,
        });
    let contains_embedded_files_name = names.is_some_and(|names| {
        names
            .get(b"EmbeddedFiles")
            .is_ok_and(|value| !matches!(value, Object::Null))
    });
    let mut file_specs_with_embedded_files = Vec::new();
    if let Some(names) = names
        && let Ok(embedded_files) = names.get(b"EmbeddedFiles")
    {
        let mut ancestors = BTreeSet::new();
        let mut steps = 0usize;
        inspect_name_tree(
            document,
            embedded_files,
            limits,
            &mut file_specs_with_embedded_files,
            &mut ancestors,
            &mut steps,
            0,
        )?;
    }
    let contains_optional_content = catalog
        .get(b"OCProperties")
        .is_ok_and(|value| !matches!(value, Object::Null));

    Ok(DocumentFeatureSummary {
        catalog_id,
        mark_info_object_id,
        mark_info_is_dictionary,
        marked,
        struct_tree_root_object_id: structure_tree.root_object_id,
        struct_tree_root_present: structure_tree.present,
        struct_tree_root_valid: structure_tree.valid,
        struct_tree_role_map_has_cycle: structure_tree.role_map_has_cycle,
        struct_tree_has_unmapped_type: structure_tree.has_unmapped_type,
        contains_embedded_files_name,
        contains_optional_content,
        file_specs_with_embedded_files,
        language_failures: structure_tree
            .language_failures
            .into_iter()
            .chain(language_failures)
            .collect(),
    })
}

#[derive(Default)]
struct StructureTreeSummary {
    root_object_id: Option<PdfObjectId>,
    present: bool,
    valid: bool,
    role_map_has_cycle: bool,
    has_unmapped_type: bool,
    structure_types: BTreeSet<Vec<u8>>,
    language_failures: Vec<RuleFailure>,
}

fn inspect_structure_tree(
    document: &Document,
    catalog: &lopdf::Dictionary,
    limits: &SafetyLimits,
) -> Result<StructureTreeSummary, PdfError> {
    let Ok(entry) = catalog.get(b"StructTreeRoot") else {
        return Ok(StructureTreeSummary::default());
    };
    if matches!(entry, Object::Null) {
        return Ok(StructureTreeSummary::default());
    }
    let root_object_id = entry.as_reference().ok().map(Into::into);
    let Some(root) = resolve_optional(document, entry, limits.max_reference_depth)? else {
        return Ok(StructureTreeSummary {
            root_object_id,
            present: true,
            ..StructureTreeSummary::default()
        });
    };
    let Some(root_dictionary) = root.as_dict().ok() else {
        return Ok(StructureTreeSummary {
            root_object_id,
            present: true,
            ..StructureTreeSummary::default()
        });
    };

    let mut summary = StructureTreeSummary {
        root_object_id,
        present: true,
        valid: true,
        role_map_has_cycle: false,
        has_unmapped_type: false,
        structure_types: BTreeSet::new(),
        language_failures: Vec::new(),
    };
    let role_map = root_dictionary
        .get(b"RoleMap")
        .ok()
        .map(|value| inspect_role_map(document, value, limits))
        .transpose()?
        .unwrap_or_default();
    summary.role_map_has_cycle = role_map.has_cycle;
    let mut ancestors = BTreeSet::new();
    if let Ok(root_id) = entry.as_reference() {
        ancestors.insert(root_id);
    }
    let mut steps = 0;
    if let Ok(kids) = root_dictionary.get(b"K") {
        inspect_structure_kids(
            document,
            kids,
            limits,
            &mut summary,
            &mut ancestors,
            &mut steps,
            0,
        )?;
    }
    summary.has_unmapped_type = summary.structure_types.iter().any(|structure_type| {
        !is_standard_structure_type(structure_type)
            && !resolves_to_standard_type(
                structure_type,
                &role_map.mappings,
                limits.max_object_count,
            )
    });
    Ok(summary)
}

#[derive(Default)]
struct RoleMapSummary {
    mappings: BTreeMap<Vec<u8>, Vec<u8>>,
    has_cycle: bool,
}

fn inspect_role_map(
    document: &Document,
    value: &Object,
    limits: &SafetyLimits,
) -> Result<RoleMapSummary, PdfError> {
    let role_map = match resolve_optional(document, value, limits.max_reference_depth) {
        Ok(Some(object)) => dictionary_based(object),
        Ok(None) | Err(PdfError::ReferenceDepth(_)) => None,
        Err(error) => return Err(error),
    };
    let Some(role_map) = role_map else {
        return Ok(RoleMapSummary::default());
    };

    let mut mappings = BTreeMap::new();
    for (entries, (source, target)) in role_map.iter().enumerate() {
        if entries >= limits.max_object_count {
            break;
        }
        let target = match resolve_optional(document, target, limits.max_reference_depth) {
            Ok(Some(object)) => object.as_name().ok(),
            Ok(None) | Err(PdfError::ReferenceDepth(_)) => None,
            Err(error) => return Err(error),
        };
        let Some(target) = target else {
            continue;
        };
        mappings.insert(source.to_vec(), target.to_vec());
    }

    let mut summary = RoleMapSummary {
        mappings,
        has_cycle: false,
    };
    for source in summary.mappings.keys() {
        let mut path = BTreeSet::new();
        let mut current = source.as_slice();
        let mut steps = 0usize;
        loop {
            if steps >= limits.max_object_count {
                break;
            }
            steps += 1;
            if !path.insert(current.to_vec()) {
                summary.has_cycle = true;
                break;
            }
            let Some(target) = summary.mappings.get(current) else {
                break;
            };
            current = target;
        }
    }
    Ok(summary)
}

fn resolves_to_standard_type(
    source: &[u8],
    mappings: &BTreeMap<Vec<u8>, Vec<u8>>,
    max_steps: usize,
) -> bool {
    let mut current = source;
    let mut steps = 0;
    while steps < max_steps {
        if is_standard_structure_type(current) {
            return true;
        }
        let Some(target) = mappings.get(current) else {
            return false;
        };
        current = target;
        steps += 1;
    }
    // A configured bound is an inspection limit, not evidence that the role
    // is invalid. The caller reports only mappings that were fully resolved.
    true
}

fn is_standard_structure_type(value: &[u8]) -> bool {
    matches!(
        value,
        b"Document"
            | b"Part"
            | b"Art"
            | b"Sect"
            | b"Div"
            | b"BlockQuote"
            | b"Caption"
            | b"TOC"
            | b"TOCI"
            | b"Index"
            | b"NonStruct"
            | b"Private"
            | b"P"
            | b"H"
            | b"H1"
            | b"H2"
            | b"H3"
            | b"H4"
            | b"H5"
            | b"H6"
            | b"L"
            | b"LI"
            | b"Lbl"
            | b"LBody"
            | b"Table"
            | b"TR"
            | b"TH"
            | b"TD"
            | b"THead"
            | b"TBody"
            | b"TFoot"
            | b"Span"
            | b"Quote"
            | b"Note"
            | b"Reference"
            | b"BibEntry"
            | b"Code"
            | b"Link"
            | b"Annot"
            | b"Ruby"
            | b"RB"
            | b"RT"
            | b"RP"
            | b"Warichu"
            | b"WT"
            | b"WP"
            | b"Figure"
            | b"Formula"
            | b"Form"
    )
}

fn inspect_structure_kids(
    document: &Document,
    value: &Object,
    limits: &SafetyLimits,
    summary: &mut StructureTreeSummary,
    ancestors: &mut BTreeSet<ObjectId>,
    steps: &mut usize,
    depth: usize,
) -> Result<(), PdfError> {
    if depth > limits.max_reference_depth {
        return Err(PdfError::ReferenceDepth(limits.max_reference_depth));
    }
    *steps += 1;
    if *steps > limits.max_object_count {
        return Err(PdfError::TooManyObjects {
            actual: *steps,
            limit: limits.max_object_count,
        });
    }
    let Some(resolved) = resolve_optional(document, value, limits.max_reference_depth)? else {
        return Ok(());
    };
    match resolved {
        Object::Integer(_) => {}
        Object::Array(values) => {
            for value in values {
                inspect_structure_kids(
                    document,
                    value,
                    limits,
                    summary,
                    ancestors,
                    steps,
                    depth + 1,
                )?;
            }
        }
        Object::Dictionary(dictionary) => {
            let structure_id = value.as_reference().ok();
            if let Some(structure_id) = structure_id
                && !ancestors.insert(structure_id)
            {
                return Err(PdfError::ReferenceDepth(limits.max_reference_depth));
            }
            let result = inspect_structure_element(
                document,
                dictionary,
                structure_id.map(Into::into),
                limits,
                summary,
                ancestors,
                steps,
                depth,
            );
            if let Some(structure_id) = structure_id {
                ancestors.remove(&structure_id);
            }
            result?;
        }
        _ => {}
    }
    Ok(())
}

fn inspect_structure_element(
    document: &Document,
    dictionary: &lopdf::Dictionary,
    object_id: Option<PdfObjectId>,
    limits: &SafetyLimits,
    summary: &mut StructureTreeSummary,
    ancestors: &mut BTreeSet<ObjectId>,
    steps: &mut usize,
    depth: usize,
) -> Result<(), PdfError> {
    if let Some(failure) = crate::language::inspect_dictionary(
        document,
        limits,
        dictionary,
        object_id,
        "structure element",
    ) {
        summary.language_failures.push(failure);
    }
    let Some(structure_type) = dictionary
        .get(b"S")
        .ok()
        .and_then(|value| value.as_name().ok())
    else {
        let kind = dictionary
            .get(b"Type")
            .ok()
            .and_then(|value| value.as_name().ok());
        if matches!(kind, Some(b"MCR") | Some(b"OBJR")) {
            return Ok(());
        }
        return Ok(());
    };
    summary.structure_types.insert(structure_type.to_vec());
    if let Ok(kids) = dictionary.get(b"K") {
        inspect_structure_kids(document, kids, limits, summary, ancestors, steps, depth + 1)?;
    }
    Ok(())
}

/// Walks one name-tree node (an intermediate node with `/Kids`, a leaf with
/// `/Names`, or both), starting from `node` before it has been resolved so
/// that a node's own indirect identity is tracked for cycle detection just
/// like every other reference this node is reached through — including the
/// tree's own root, unlike a walker that only registers references
/// encountered while iterating `/Kids`. This mirrors `page_tree::collect_pages`'s
/// traversal shape so both trees share one cycle-safety story: `ancestors`
/// tracks only ids currently on the path from the root to this call (pushed
/// on entry, popped before every return), so a *true* cycle (an id
/// revisited while still an ancestor) raises `PdfError::ReferenceDepth`,
/// while the same file specification or intermediate node legitimately
/// reachable from two different `Kids` branches (a DAG, not a cycle) is not
/// mistaken for one — confirmed against veraPDF 1.30.2, which processes
/// such a shared reference without a parse or resource-limit failure, the
/// same way it does for a page shared by two `Pages` branches. `steps`
/// bounds the walk's total work (independent of ancestor depth) against a
/// DAG that fans out shared subtrees without any node being its own
/// ancestor, the same DAG-blowup safety `page_tree.rs` uses.
fn inspect_name_tree(
    document: &Document,
    node: &Object,
    limits: &SafetyLimits,
    failures: &mut Vec<RuleFailure>,
    ancestors: &mut BTreeSet<ObjectId>,
    steps: &mut usize,
    depth: usize,
) -> Result<(), PdfError> {
    if depth > limits.max_reference_depth {
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
    let result = inspect_name_tree_node(document, node, limits, failures, ancestors, steps, depth);
    if let Some(id) = object_id {
        ancestors.remove(&id);
    }
    result
}

fn inspect_name_tree_node(
    document: &Document,
    node: &Object,
    limits: &SafetyLimits,
    failures: &mut Vec<RuleFailure>,
    ancestors: &mut BTreeSet<ObjectId>,
    steps: &mut usize,
    depth: usize,
) -> Result<(), PdfError> {
    let Some(node) =
        resolve_optional(document, node, limits.max_reference_depth)?.and_then(dictionary_based)
    else {
        return Ok(());
    };
    if let Ok(names) = node.get(b"Names")
        && let Some(names) = resolve_optional(document, names, limits.max_reference_depth)?
            .and_then(|object| object.as_array().ok())
    {
        for value in names.iter().skip(1).step_by(2) {
            if let Some(failure) = file_spec::inspect(
                document,
                value,
                limits,
                "file specification in the EmbeddedFiles name tree",
            )? {
                failures.push(failure);
            }
        }
    }
    if let Ok(kids) = node.get(b"Kids")
        && let Some(kids) = resolve_optional(document, kids, limits.max_reference_depth)?
            .and_then(|object| object.as_array().ok())
    {
        for value in kids {
            inspect_name_tree(
                document,
                value,
                limits,
                failures,
                ancestors,
                steps,
                depth + 1,
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use lopdf::{Document, Object, dictionary};

    use super::inspect;
    use crate::{PdfError, SafetyLimits};

    #[test]
    fn rejects_cyclic_embedded_files_name_tree() {
        let mut document = Document::with_version("1.4");
        document.trailer.set("Root", Object::Reference((1, 0)));
        document.objects.insert(
            (1, 0),
            Object::Dictionary(dictionary! {
                "Type" => "Catalog",
                "Names" => Object::Reference((2, 0)),
            }),
        );
        document.objects.insert(
            (2, 0),
            Object::Dictionary(dictionary! { "EmbeddedFiles" => Object::Reference((3, 0)) }),
        );
        document.objects.insert(
            (3, 0),
            Object::Dictionary(dictionary! { "Kids" => vec![Object::Reference((4, 0))] }),
        );
        document.objects.insert(
            (4, 0),
            Object::Dictionary(dictionary! { "Kids" => vec![Object::Reference((3, 0))] }),
        );

        assert!(matches!(
            inspect(&document, &SafetyLimits::default()),
            Err(PdfError::ReferenceDepth(_))
        ));
    }

    #[test]
    fn rejects_an_embedded_files_node_that_references_itself() {
        let mut document = Document::with_version("1.4");
        document.trailer.set("Root", Object::Reference((1, 0)));
        document.objects.insert(
            (1, 0),
            Object::Dictionary(dictionary! {
                "Type" => "Catalog",
                "Names" => Object::Reference((2, 0)),
            }),
        );
        document.objects.insert(
            (2, 0),
            Object::Dictionary(dictionary! { "EmbeddedFiles" => Object::Reference((3, 0)) }),
        );
        // The EmbeddedFiles node's own Kids loops straight back to itself,
        // a one-hop self-reference at the tree's root rather than a cycle
        // spanning several Kids hops.
        document.objects.insert(
            (3, 0),
            Object::Dictionary(dictionary! { "Kids" => vec![Object::Reference((3, 0))] }),
        );

        assert!(matches!(
            inspect(&document, &SafetyLimits::default()),
            Err(PdfError::ReferenceDepth(_))
        ));
    }

    #[test]
    fn accepts_a_valid_structure_element_tree() {
        let mut document = Document::with_version("1.4");
        document.trailer.set("Root", Object::Reference((1, 0)));
        document.objects.insert(
            (1, 0),
            Object::Dictionary(dictionary! {
                "Type" => "Catalog",
                "StructTreeRoot" => Object::Reference((2, 0)),
            }),
        );
        document.objects.insert(
            (2, 0),
            Object::Dictionary(dictionary! { "K" => vec![Object::Reference((3, 0))] }),
        );
        document.objects.insert(
            (3, 0),
            Object::Dictionary(dictionary! {
                "S" => "P",
                "P" => Object::Reference((2, 0)),
                "K" => vec![Object::Reference((4, 0))],
            }),
        );
        document
            .objects
            .insert((4, 0), Object::Dictionary(dictionary! { "S" => "Span" }));

        let features = inspect(&document, &SafetyLimits::default()).expect("inspect");
        assert!(features.struct_tree_root_present);
        assert!(features.struct_tree_root_valid);
    }

    /// Confirmed against veraPDF 1.30.2: the same name-tree leaf reachable
    /// from two different `Kids` branches (a DAG, not a cycle — neither
    /// branch is the other's ancestor) is processed without error, the same
    /// way a Page object shared by two `Pages` branches is (see
    /// `page_tree.rs`'s identical fix this session).
    #[test]
    fn shared_name_tree_leaf_reached_from_two_branches_is_not_a_cycle() {
        let mut document = Document::with_version("1.4");
        document.trailer.set("Root", Object::Reference((1, 0)));
        document.objects.insert(
            (1, 0),
            Object::Dictionary(dictionary! {
                "Type" => "Catalog",
                "Names" => Object::Reference((2, 0)),
            }),
        );
        document.objects.insert(
            (2, 0),
            Object::Dictionary(dictionary! { "EmbeddedFiles" => Object::Reference((3, 0)) }),
        );
        // Root EmbeddedFiles node with two Kids branches (4,0) and (5,0),
        // both pointing at the same leaf (6,0).
        document.objects.insert(
            (3, 0),
            Object::Dictionary(
                dictionary! { "Kids" => vec![Object::Reference((4, 0)), Object::Reference((5, 0))] },
            ),
        );
        document.objects.insert(
            (4, 0),
            Object::Dictionary(dictionary! { "Kids" => vec![Object::Reference((6, 0))] }),
        );
        document.objects.insert(
            (5, 0),
            Object::Dictionary(dictionary! { "Kids" => vec![Object::Reference((6, 0))] }),
        );
        document.objects.insert(
            (6, 0),
            Object::Dictionary(dictionary! {
                "Names" => vec![
                    Object::string_literal("file"),
                    Object::Dictionary(dictionary! {}),
                ],
            }),
        );

        assert!(inspect(&document, &SafetyLimits::default()).is_ok());
    }
}
