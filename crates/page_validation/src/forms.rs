use std::collections::BTreeSet;

use lopdf::{Dictionary, Document, Object, ObjectId};
use roxmltree::Document as XmlDocument;

use crate::annotations::{annotation_is_outside_crop_box, annotation_structure_element};
use crate::catalog::resolve_catalog;
use crate::content_support::for_each_page_annotation;
use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::object_resolution::{
    contains_key, dictionary_based, has_non_empty_string_entry, resolve_optional, resolved_integer,
    resolved_name, walk_inherited,
};
use crate::page_tree::PageEntry;
use crate::report::RuleFailure;

#[derive(Clone, Debug, Default)]
pub(crate) struct FormSummary {
    pub(crate) invalid_need_appearances: Vec<RuleFailure>,
    pub(crate) widgets_without_appearances: Vec<RuleFailure>,
    pub(crate) widgets_missing_tu_or_alt: Vec<RuleFailure>,
    pub(crate) tu_language_failures: Vec<RuleFailure>,
    pub(crate) dynamic_xfa_forms: Vec<RuleFailure>,
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
    if xfa_is_dynamic(document, acro_form.get(b"XFA").ok(), limits)? {
        summary.dynamic_xfa_forms.push(RuleFailure {
            object_id,
            description: "the catalog AcroForm contains dynamic XFA".to_owned(),
        });
    }
    let catalog_contains_lang = contains_key(catalog, b"Lang");
    for_each_form_field(
        document,
        acro_form,
        limits,
        |field, object_id, context, _depth| {
            if has_non_null_entry(document, field, b"TU", limits)? && !catalog_contains_lang {
                summary.tu_language_failures.push(RuleFailure {
                    object_id,
                    description: format!("{context} has /TU without a catalog /Lang"),
                });
            }
            Ok(())
        },
    )?;
    Ok(())
}

fn xfa_is_dynamic(
    document: &Document,
    xfa: Option<&Object>,
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    let Some(xfa) = xfa else {
        return Ok(false);
    };
    let Some(xfa) = resolve_optional(document, xfa, limits.max_reference_depth)? else {
        return Ok(false);
    };
    match xfa {
        Object::Stream(stream) => xfa_stream_is_dynamic(stream, limits),
        Object::Array(packets) => {
            let mut config_stream = None;
            let mut stream_entries = Vec::new();
            for (index, packet) in packets.iter().enumerate() {
                let Some(packet) = resolve_optional(document, packet, limits.max_reference_depth)?
                else {
                    continue;
                };
                if index % 2 == 0 {
                    if packet.as_str().ok() == Some(b"config") {
                        config_stream = packets.get(index + 1);
                    }
                } else if packet.as_stream().is_ok() {
                    stream_entries.push(packet);
                }
            }
            if let Some(config_stream) = config_stream
                && let Some(config_stream) =
                    resolve_optional(document, config_stream, limits.max_reference_depth)?
                && let Ok(config_stream) = config_stream.as_stream()
            {
                return xfa_stream_is_dynamic(config_stream, limits);
            }
            for packet in stream_entries {
                if let Ok(stream) = packet.as_stream()
                    && xfa_stream_is_dynamic(stream, limits)?
                {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}

fn xfa_stream_is_dynamic(stream: &lopdf::Stream, limits: &SafetyLimits) -> Result<bool, PdfError> {
    let bytes = match stream.decompressed_content_with_limit(limits.max_decoded_stream_size) {
        Ok(bytes) => bytes,
        Err(lopdf::Error::Decompress(lopdf::DecompressError::MemoryLimitExceeded { .. })) => {
            return Err(PdfError::XfaDecodeLimit(limits.max_decoded_stream_size));
        }
        Err(_) => return Ok(false),
    };
    let Ok(xml) = std::str::from_utf8(&bytes) else {
        return Ok(false);
    };
    let Ok(document) = XmlDocument::parse(xml) else {
        return Ok(false);
    };
    Ok(document.descendants().any(|node| {
        node.tag_name().name() == "dynamicRender"
            && node.text().is_some_and(|value| value == "required")
    }))
}

/// Visits every form field modeled from an AcroForm `/Fields` array and its
/// named `/Kids`, sharing the field reachability rules used by action checks.
pub(crate) fn for_each_form_field(
    document: &Document,
    acro_form: &Dictionary,
    limits: &SafetyLimits,
    mut visit: impl FnMut(&Dictionary, Option<PdfObjectId>, &str, usize) -> Result<(), PdfError>,
) -> Result<(), PdfError> {
    let Some(fields) = acro_form
        .get(b"Fields")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|object| object.as_array().ok())
    else {
        return Ok(());
    };
    let mut seen_fields = BTreeSet::new();
    for (index, field) in fields.iter().enumerate() {
        visit_form_field(
            document,
            field,
            &format!("AcroForm field {index}"),
            0,
            true,
            limits,
            &mut seen_fields,
            &mut visit,
        )?;
    }
    Ok(())
}

fn visit_form_field(
    document: &Document,
    value: &Object,
    context: &str,
    depth: usize,
    top_level: bool,
    limits: &SafetyLimits,
    seen_fields: &mut BTreeSet<ObjectId>,
    visit: &mut impl FnMut(&Dictionary, Option<PdfObjectId>, &str, usize) -> Result<(), PdfError>,
) -> Result<(), PdfError> {
    if depth > limits.max_reference_depth {
        return Err(PdfError::ReferenceDepth(limits.max_reference_depth));
    }
    let object_id = value.as_reference().ok();
    let Some(field) =
        resolve_optional(document, value, limits.max_reference_depth)?.and_then(dictionary_based)
    else {
        return Ok(());
    };
    if !top_level && !contains_key(field, b"T") {
        return Ok(());
    }
    if object_id.is_some_and(|id| !seen_fields.insert(id)) {
        return Ok(());
    }
    visit(field, object_id.map(Into::into), context, depth)?;
    let Some(kids) = field
        .get(b"Kids")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|object| object.as_array().ok())
    else {
        return Ok(());
    };
    for (index, kid) in kids.iter().enumerate() {
        visit_form_field(
            document,
            kid,
            &format!("{context} child {index}"),
            depth + 1,
            false,
            limits,
            seen_fields,
            visit,
        )?;
    }
    Ok(())
}

fn has_non_null_entry(
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
            .is_some_and(|value| !matches!(value, Object::Null)),
    )
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
        |page_number, index, object_id, page, value| {
            let Ok(annotation) = value.as_dict() else {
                return Ok(());
            };
            if resolved_name(document, annotation, b"Subtype", limits.max_reference_depth)?
                != Some(b"Widget".as_slice())
            {
                return Ok(());
            }
            let hidden = resolved_integer(document, annotation, b"F", limits.max_reference_depth)?
                .is_some_and(|flags| flags & 2 == 2);
            let outside_crop_box =
                annotation_is_outside_crop_box(document, page, annotation, limits)?;
            let has_alt = annotation_structure_element(document, annotation, limits)?
                .map(|structure_element| {
                    has_non_empty_string_entry(
                        document,
                        structure_element,
                        b"Alt",
                        limits.max_reference_depth,
                    )
                })
                .transpose()?
                .unwrap_or(false);
            if !hidden
                && !outside_crop_box
                && !widget_has_non_empty_tu(document, annotation, limits)?
                && !has_alt
            {
                summary.widgets_missing_tu_or_alt.push(RuleFailure {
                    object_id,
                    description: format!(
                        "Widget annotation {index} on page {page_number} has neither a non-empty inherited /TU entry nor a non-empty /Alt entry in its enclosing structure element"
                    ),
                });
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

fn widget_has_non_empty_tu(
    document: &Document,
    widget: &Dictionary,
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    Ok(walk_inherited(
        document,
        widget,
        limits,
        b"TU",
        |document, value, limits| {
            Ok(Some(
                resolve_optional(document, value, limits.max_reference_depth)?
                    .and_then(|value| value.as_str().ok())
                    .is_some_and(|value| !value.is_empty()),
            ))
        },
    )?
    .unwrap_or(false))
}

fn object_number(value: &Object) -> Option<f64> {
    value
        .as_i64()
        .map(|value| value as f64)
        .or_else(|_| value.as_float().map(f64::from))
        .ok()
}
