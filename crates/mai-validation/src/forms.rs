use std::collections::BTreeSet;

use lopdf::{Document, Object};

use crate::content_support::for_each_page_annotation;
use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::object_resolution::{dictionary_based, resolve_optional};
use crate::report::RuleFailure;

#[derive(Clone, Debug, Default)]
pub(crate) struct FormSummary {
    pub(crate) invalid_need_appearances: Vec<RuleFailure>,
    pub(crate) widgets_without_appearances: Vec<RuleFailure>,
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
        summary.invalid_need_appearances.push(RuleFailure {
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
    for_each_page_annotation(
        document,
        limits,
        &mut inspected,
        |page_number, index, object_id, value| {
            let Some(annotation) = dictionary_based(value) else {
                return Ok(());
            };
            if annotation
                .get(b"Subtype")
                .ok()
                .and_then(|value| value.as_name().ok())
                != Some(b"Widget".as_slice())
            {
                return Ok(());
            }
            let has_appearance = annotation
                .get(b"AP")
                .ok()
                .map(|value| resolve_optional(document, value, limits.max_reference_depth))
                .transpose()?
                .flatten()
                .is_some_and(|object| object.as_dict().is_ok());
            if !has_appearance {
                summary.widgets_without_appearances.push(RuleFailure {
                    object_id,
                    description: format!(
                        "Widget annotation {index} on page {page_number} has no appearance dictionary"
                    ),
                });
            }
            Ok(())
        },
    )
}
