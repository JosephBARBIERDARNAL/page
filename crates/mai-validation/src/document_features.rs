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

    let contains_embedded_files_name = catalog
        .get(b"Names")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|object| match object {
            Object::Dictionary(dictionary) => Some(dictionary),
            _ => None,
        })
        .is_some_and(|names| {
            names
                .get(b"EmbeddedFiles")
                .is_ok_and(|value| !matches!(value, Object::Null))
        });
    let contains_optional_content = catalog
        .get(b"OCProperties")
        .is_ok_and(|value| !matches!(value, Object::Null));

    Ok(DocumentFeatureSummary {
        catalog_id,
        contains_embedded_files_name,
        contains_optional_content,
    })
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
