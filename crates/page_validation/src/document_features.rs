use std::collections::BTreeSet;

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
    pub(crate) contains_embedded_files_name: bool,
    pub(crate) contains_optional_content: bool,
    pub(crate) file_specs_with_embedded_files: Vec<RuleFailure>,
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
        contains_embedded_files_name,
        contains_optional_content,
        file_specs_with_embedded_files,
    })
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
#[allow(clippy::too_many_arguments)]
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

#[allow(clippy::too_many_arguments)]
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
