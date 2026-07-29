use std::collections::BTreeSet;

use lopdf::{Document, Object};

use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;

#[derive(Clone, Debug, Default)]
pub(crate) struct DocumentFeatureSummary {
    pub(crate) catalog_id: Option<PdfObjectId>,
    pub(crate) contains_embedded_files_name: bool,
    pub(crate) contains_optional_content: bool,
    pub(crate) file_specs_with_embedded_files: Vec<FileSpecFailure>,
}

#[derive(Clone, Debug)]
pub(crate) struct FileSpecFailure {
    pub(crate) object_id: Option<PdfObjectId>,
}

pub(crate) fn inspect(
    document: &Document,
    limits: &SafetyLimits,
) -> Result<DocumentFeatureSummary, PdfError> {
    let root = document.trailer.get(b"Root").ok();
    let catalog_id = root
        .and_then(|value| value.as_reference().ok())
        .map(Into::into);
    let Some(catalog) = root
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|object| match object {
            Object::Dictionary(dictionary) => Some(dictionary),
            _ => None,
        })
    else {
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
        && let Some(embedded_files) =
            resolve_optional(document, embedded_files, limits.max_reference_depth)?
                .and_then(dictionary_based)
    {
        inspect_name_tree(
            document,
            embedded_files,
            limits,
            &mut file_specs_with_embedded_files,
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

fn inspect_name_tree(
    document: &Document,
    node: &lopdf::Dictionary,
    limits: &SafetyLimits,
    failures: &mut Vec<FileSpecFailure>,
) -> Result<(), PdfError> {
    if let Ok(names) = node.get(b"Names")
        && let Some(names) = resolve_optional(document, names, limits.max_reference_depth)?
            .and_then(|object| object.as_array().ok())
    {
        for value in names.iter().skip(1).step_by(2) {
            let object_id = value.as_reference().ok().map(Into::into);
            if resolve_optional(document, value, limits.max_reference_depth)?
                .and_then(dictionary_based)
                .is_some_and(|file_spec| {
                    file_spec
                        .get(b"EF")
                        .is_ok_and(|value| !matches!(value, Object::Null))
                })
            {
                failures.push(FileSpecFailure { object_id });
            }
        }
    }
    if let Ok(kids) = node.get(b"Kids")
        && let Some(kids) = resolve_optional(document, kids, limits.max_reference_depth)?
            .and_then(|object| object.as_array().ok())
    {
        for value in kids {
            if let Some(child) = resolve_optional(document, value, limits.max_reference_depth)?
                .and_then(dictionary_based)
            {
                inspect_name_tree(document, child, limits, failures)?;
            }
        }
    }
    Ok(())
}

fn dictionary_based(object: &Object) -> Option<&lopdf::Dictionary> {
    match object {
        Object::Dictionary(dictionary) => Some(dictionary),
        Object::Stream(stream) => Some(&stream.dict),
        _ => None,
    }
}

fn resolve<'a>(
    document: &'a Document,
    mut object: &'a Object,
    maximum_depth: usize,
) -> Result<&'a Object, PdfError> {
    let mut visited = BTreeSet::new();
    for _ in 0..=maximum_depth {
        let Object::Reference(object_id) = object else {
            return Ok(object);
        };
        if !visited.insert(*object_id) {
            return Err(PdfError::ReferenceDepth(maximum_depth));
        }
        object = document
            .objects
            .get(object_id)
            .ok_or(PdfError::UnexpectedObject("missing indirect object"))?;
    }
    Err(PdfError::ReferenceDepth(maximum_depth))
}

fn resolve_optional<'a>(
    document: &'a Document,
    object: &'a Object,
    maximum_depth: usize,
) -> Result<Option<&'a Object>, PdfError> {
    match resolve(document, object, maximum_depth) {
        Ok(object) => Ok(Some(object)),
        Err(error @ PdfError::ReferenceDepth(_)) => Err(error),
        Err(_) => Ok(None),
    }
}
