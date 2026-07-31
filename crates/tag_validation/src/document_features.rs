use std::collections::BTreeSet;

use lopdf::{Document, Object, ObjectId};

use crate::catalog::resolve_catalog;
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
    let root = document.trailer.get(b"Root").ok();
    let catalog_id = root
        .and_then(|value| value.as_reference().ok())
        .map(Into::into);
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
        let mut visited = BTreeSet::new();
        inspect_name_tree(
            document,
            embedded_files,
            limits,
            &mut file_specs_with_embedded_files,
            &mut visited,
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
/// traversal shape so both trees share one cycle-safety story: any indirect
/// object id revisited during the walk raises `PdfError::ReferenceDepth`
/// rather than silently truncating traversal.
fn inspect_name_tree(
    document: &Document,
    node: &Object,
    limits: &SafetyLimits,
    failures: &mut Vec<RuleFailure>,
    visited: &mut BTreeSet<ObjectId>,
    depth: usize,
) -> Result<(), PdfError> {
    if depth > limits.max_reference_depth {
        return Err(PdfError::ReferenceDepth(limits.max_reference_depth));
    }
    if let Ok(object_id) = node.as_reference()
        && !visited.insert(object_id)
    {
        return Err(PdfError::ReferenceDepth(limits.max_reference_depth));
    }
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
            inspect_name_tree(document, value, limits, failures, visited, depth + 1)?;
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
}
