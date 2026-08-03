use std::collections::{BTreeMap, BTreeSet};

use lopdf::{Dictionary, Document};

use crate::content_support::for_each_page_annotation;
use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::object_resolution::{resolve_optional, resolved_integer, resolved_name, walk_inherited};
use crate::page_tree::PageEntry;
use crate::report::RuleFailure;

#[derive(Clone, Debug, Default)]
pub(crate) struct AnnotationSummary {
    pub(crate) invalid_subtypes: Vec<RuleFailure>,
    pub(crate) invalid_opacities: Vec<RuleFailure>,
    pub(crate) invalid_flags: Vec<RuleFailure>,
    pub(crate) color_uses: Vec<RuleFailure>,
    pub(crate) invalid_appearance_entries: Vec<RuleFailure>,
    pub(crate) invalid_button_appearances: Vec<RuleFailure>,
    pub(crate) invalid_other_appearances: Vec<RuleFailure>,
}

pub(crate) fn inspect(
    document: &Document,
    pages: &BTreeMap<u32, PageEntry>,
    limits: &SafetyLimits,
) -> Result<AnnotationSummary, PdfError> {
    let mut summary = AnnotationSummary::default();
    let mut inspected = BTreeSet::new();
    for_each_page_annotation(
        document,
        pages,
        limits,
        &mut inspected,
        |page_number, index, object_id, annotation| {
            let Some(dictionary) = annotation.as_dict().ok() else {
                return Ok(());
            };
            inspect_annotation(
                document,
                dictionary,
                object_id,
                &format!("annotation {index} on page {page_number}"),
                limits,
                &mut summary,
            )
        },
    )?;
    Ok(summary)
}

fn inspect_annotation(
    document: &Document,
    annotation: &Dictionary,
    object_id: Option<PdfObjectId>,
    context: &str,
    limits: &SafetyLimits,
    summary: &mut AnnotationSummary,
) -> Result<(), PdfError> {
    let subtype = resolved_name(document, annotation, b"Subtype", limits.max_reference_depth)?;
    if !matches!(
        subtype,
        Some(
            b"Text"
                | b"Link"
                | b"FreeText"
                | b"Line"
                | b"Square"
                | b"Circle"
                | b"Highlight"
                | b"Underline"
                | b"Squiggly"
                | b"StrikeOut"
                | b"Stamp"
                | b"Ink"
                | b"Popup"
                | b"Widget"
                | b"PrinterMark"
                | b"TrapNet"
        )
    ) {
        summary.invalid_subtypes.push(annotation_failure(
            object_id,
            context,
            "has a missing or forbidden /Subtype",
        ));
    }

    if annotation
        .get(b"CA")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|value| value.as_float().ok())
        .is_some_and(|value| value != 1.0)
    {
        summary.invalid_opacities.push(annotation_failure(
            object_id,
            context,
            "has /CA other than 1",
        ));
    }

    let flags = resolved_integer(document, annotation, b"F", limits.max_reference_depth)?;
    if !flags
        .is_some_and(|flags| flags & 4 == 4 && flags & 1 == 0 && flags & 2 == 0 && flags & 32 == 0)
    {
        summary.invalid_flags.push(annotation_failure(
            object_id,
            context,
            "is not printable or is hidden, invisible, or not viewable",
        ));
    }

    if contains_array(document, annotation, b"C", limits)?
        || contains_array(document, annotation, b"IC", limits)?
    {
        summary
            .color_uses
            .push(annotation_failure(object_id, context, "contains /C or /IC"));
    }

    let Ok(appearance) = annotation.get(b"AP") else {
        return Ok(());
    };
    let Some(appearance) = resolve_optional(document, appearance, limits.max_reference_depth)?
        .and_then(|object| object.as_dict().ok())
    else {
        return Ok(());
    };
    let only_normal =
        appearance.len() == 1 && appearance.iter().next().is_some_and(|(key, _)| key == b"N");
    if !only_normal {
        summary.invalid_appearance_entries.push(annotation_failure(
            object_id,
            context,
            "has an /AP dictionary with entries other than only /N",
        ));
        return Ok(());
    }

    let normal = appearance
        .get(b"N")
        .expect("single normal appearance entry");
    let normal = resolve_optional(document, normal, limits.max_reference_depth)?;
    let field_type = inherited_field_type(document, annotation, limits)?;
    if subtype == Some(b"Widget".as_slice()) && field_type == Some(b"Btn".as_slice()) {
        if normal
            .and_then(|object| object.as_dict().ok())
            .map(|states| contains_appearance_stream(document, states, limits))
            .transpose()?
            .is_none_or(|contains_stream| !contains_stream)
        {
            summary.invalid_button_appearances.push(annotation_failure(
                object_id,
                context,
                "is a button Widget whose normal appearance is not a nonempty subdictionary",
            ));
        }
    } else if normal.is_none_or(|object| object.as_stream().is_err()) {
        summary.invalid_other_appearances.push(annotation_failure(
            object_id,
            context,
            "has a normal appearance that is not a stream",
        ));
    }
    Ok(())
}

fn inherited_field_type<'a>(
    document: &'a Document,
    dictionary: &'a Dictionary,
    limits: &SafetyLimits,
) -> Result<Option<&'a [u8]>, PdfError> {
    walk_inherited(document, dictionary, limits, b"FT", |_, value, _| {
        Ok(
            resolve_optional(document, value, limits.max_reference_depth)?
                .and_then(|value| value.as_name().ok()),
        )
    })
}

fn contains_array(
    document: &Document,
    dictionary: &Dictionary,
    key: &[u8],
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    let Ok(value) = dictionary.get(key) else {
        return Ok(false);
    };
    Ok(
        resolve_optional(document, value, limits.max_reference_depth)?
            .is_some_and(|value| value.as_array().is_ok()),
    )
}

fn contains_appearance_stream(
    document: &Document,
    states: &Dictionary,
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    for (_, value) in states.iter() {
        if resolve_optional(document, value, limits.max_reference_depth)?
            .is_some_and(|value| value.as_stream().is_ok())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn annotation_failure(object_id: Option<PdfObjectId>, context: &str, detail: &str) -> RuleFailure {
    RuleFailure {
        object_id,
        description: format!("{context} {detail}"),
    }
}
