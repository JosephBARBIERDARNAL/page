use std::collections::BTreeSet;

use lopdf::{Document, Object};

use crate::catalog::resolve_catalog;
use crate::content_support::for_each_page_annotation;
use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::object_resolution::{dictionary_based, resolve_optional, resolved_name};
use crate::page_tree::PageEntry;
use crate::report::RuleFailure;

#[derive(Clone, Debug, Default)]
pub(crate) struct FormSummary {
    pub(crate) invalid_need_appearances: Vec<RuleFailure>,
    pub(crate) widgets_without_appearances: Vec<RuleFailure>,
}

pub(crate) fn inspect(
    document: &Document,
    pages: &[PageEntry],
    limits: &SafetyLimits,
) -> Result<FormSummary, PdfError> {
    let mut summary = FormSummary::default();
    inspect_acro_form(document, limits, &mut summary)?;
    inspect_page_widgets(document, pages, limits, &mut summary)?;
    Ok(summary)
}

fn inspect_acro_form(
    document: &Document,
    limits: &SafetyLimits,
    summary: &mut FormSummary,
) -> Result<(), PdfError> {
    let Some(catalog) = resolve_catalog(document, limits)? else {
        return Ok(());
    };
    let catalog = catalog.dictionary;
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
    pages: &[PageEntry],
    limits: &SafetyLimits,
    summary: &mut FormSummary,
) -> Result<(), PdfError> {
    let mut inspected = BTreeSet::new();
    for_each_page_annotation(
        document,
        pages,
        limits,
        &mut inspected,
        |page_number, index, object_id, value| {
            let Ok(annotation) = value.as_dict() else {
                return Ok(());
            };
            if resolved_name(document, annotation, b"Subtype", limits.max_reference_depth)?
                != Some(b"Widget".as_slice())
            {
                return Ok(());
            }
            let zero_rect = annotation
                .get(b"Rect")
                .ok()
                .map(|value| resolve_optional(document, value, limits.max_reference_depth))
                .transpose()?
                .flatten()
                .and_then(|value| value.as_array().ok())
                .is_some_and(|rect| {
                    let [left, bottom, right, top] = rect.as_slice() else {
                        return false;
                    };
                    matches!(
                        (
                            object_number(left),
                            object_number(bottom),
                            object_number(right),
                            object_number(top),
                        ),
                        (Some(left), Some(bottom), Some(right), Some(top))
                            if left == right && bottom == top
                    )
                });
            let has_appearance = annotation
                .get(b"AP")
                .ok()
                .map(|value| resolve_optional(document, value, limits.max_reference_depth))
                .transpose()?
                .flatten()
                .is_some_and(|object| object.as_dict().is_ok());
            if !zero_rect && !has_appearance {
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

fn object_number(value: &Object) -> Option<f64> {
    value
        .as_i64()
        .map(|value| value as f64)
        .or_else(|_| value.as_float().map(f64::from))
        .ok()
}
