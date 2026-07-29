use std::collections::BTreeSet;

use lopdf::{Dictionary, Document, Object};

use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;

#[derive(Clone, Debug, Default)]
pub(crate) struct FormSummary {
    pub(crate) invalid_need_appearances: Vec<FormFailure>,
    pub(crate) widgets_without_appearances: Vec<FormFailure>,
}

#[derive(Clone, Debug)]
pub(crate) struct FormFailure {
    pub(crate) object_id: Option<PdfObjectId>,
    pub(crate) description: String,
}

pub(crate) fn inspect(document: &Document, limits: &SafetyLimits) -> Result<FormSummary, PdfError> {
    let mut summary = FormSummary::default();
    inspect_acro_form(document, limits, &mut summary)?;
    inspect_page_widgets(document, limits, &mut summary)?;
    Ok(summary)
}

fn inspect_acro_form(
    document: &Document,
    limits: &SafetyLimits,
    summary: &mut FormSummary,
) -> Result<(), PdfError> {
    let Some(catalog) = document
        .trailer
        .get(b"Root")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|object| object.as_dict().ok())
    else {
        return Ok(());
    };
    let Ok(value) = catalog.get(b"AcroForm") else {
        return Ok(());
    };
    let object_id = value.as_reference().ok().map(Into::into);
    let Some(acro_form) =
        resolve_optional(document, value, limits.max_reference_depth)?.and_then(dictionary_based)
    else {
        return Ok(());
    };
    let invalid = match acro_form.get(b"NeedAppearances") {
        Err(_) => false,
        Ok(value) => match resolve_optional(document, value, limits.max_reference_depth)? {
            None | Some(Object::Null) | Some(Object::Boolean(false)) => false,
            Some(_) => true,
        },
    };
    if invalid {
        summary.invalid_need_appearances.push(FormFailure {
            object_id,
            description: "the catalog AcroForm has /NeedAppearances true or a non-boolean value"
                .to_owned(),
        });
    }
    Ok(())
}

fn inspect_page_widgets(
    document: &Document,
    limits: &SafetyLimits,
    summary: &mut FormSummary,
) -> Result<(), PdfError> {
    let mut inspected = BTreeSet::new();
    for (page_number, page_id) in document.get_pages() {
        let Some(page) = document
            .objects
            .get(&page_id)
            .and_then(|object| object.as_dict().ok())
        else {
            continue;
        };
        let Some(annotations) = page
            .get(b"Annots")
            .ok()
            .map(|value| resolve_optional(document, value, limits.max_reference_depth))
            .transpose()?
            .flatten()
            .and_then(|object| object.as_array().ok())
        else {
            continue;
        };
        for (index, value) in annotations.iter().enumerate() {
            let object_id = value.as_reference().ok();
            if object_id.is_some_and(|id| !inspected.insert(id)) {
                continue;
            }
            let Some(annotation) = resolve_optional(document, value, limits.max_reference_depth)?
                .and_then(dictionary_based)
            else {
                continue;
            };
            if annotation
                .get(b"Subtype")
                .ok()
                .and_then(|value| value.as_name().ok())
                != Some(b"Widget".as_slice())
            {
                continue;
            }
            let has_appearance = annotation
                .get(b"AP")
                .ok()
                .map(|value| resolve_optional(document, value, limits.max_reference_depth))
                .transpose()?
                .flatten()
                .is_some_and(|object| object.as_dict().is_ok());
            if !has_appearance {
                summary.widgets_without_appearances.push(FormFailure {
                    object_id: object_id.map(Into::into),
                    description: format!(
                        "Widget annotation {index} on page {page_number} has no appearance dictionary"
                    ),
                });
            }
        }
    }
    Ok(())
}

fn dictionary_based(object: &Object) -> Option<&Dictionary> {
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
