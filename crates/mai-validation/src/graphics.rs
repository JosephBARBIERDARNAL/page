use lopdf::{Dictionary, Document, ObjectId};

use crate::error::PdfError;
use crate::icc_based::IccBasedSummary;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::object_resolution::{dictionary_based, resolve_optional};
use crate::report::RuleFailure;

#[derive(Clone, Debug, Default)]
pub(crate) struct GraphicsSummary {
    pub(crate) transfer_functions: Vec<RuleFailure>,
    pub(crate) transfer_functions_2: Vec<RuleFailure>,
    pub(crate) rendering_intents: Vec<RuleFailure>,
    pub(crate) extgstate_soft_masks: Vec<RuleFailure>,
    pub(crate) xobject_soft_masks: Vec<RuleFailure>,
    pub(crate) transparency_groups: Vec<RuleFailure>,
    pub(crate) blend_modes: Vec<RuleFailure>,
    pub(crate) stroke_alpha: Vec<RuleFailure>,
    pub(crate) fill_alpha: Vec<RuleFailure>,
}

pub(crate) fn inspect(
    document: &Document,
    content: &IccBasedSummary,
    limits: &SafetyLimits,
) -> Result<GraphicsSummary, PdfError> {
    let mut summary = GraphicsSummary::default();
    for (name, context) in &content.invalid_rendering_intents {
        summary.rendering_intents.push(RuleFailure {
            object_id: None,
            description: format!("content in {context} uses rendering intent /{name}"),
        });
    }

    for object_id in &content.used_extgstate_ids {
        let Some(dictionary) = resolve_dictionary(document, *object_id, limits)? else {
            continue;
        };
        let reported_id = Some((*object_id).into());
        if dictionary.has(b"TR") {
            summary.transfer_functions.push(RuleFailure {
                object_id: reported_id,
                description: "used ExtGState dictionary contains /TR".to_owned(),
            });
        }
        if dictionary.has(b"TR2")
            && dictionary
                .get(b"TR2")
                .ok()
                .and_then(|value| value.as_name().ok())
                != Some(b"Default".as_slice())
        {
            summary.transfer_functions_2.push(RuleFailure {
                object_id: reported_id,
                description: "used ExtGState dictionary has /TR2 other than /Default".to_owned(),
            });
        }
        if dictionary.has(b"SMask")
            && dictionary
                .get(b"SMask")
                .ok()
                .and_then(|value| value.as_name().ok())
                != Some(b"None".as_slice())
        {
            summary.extgstate_soft_masks.push(RuleFailure {
                object_id: reported_id,
                description: "used ExtGState dictionary has /SMask other than /None".to_owned(),
            });
        }
        if dictionary.has(b"BM")
            && !matches!(
                dictionary
                    .get(b"BM")
                    .ok()
                    .and_then(|value| value.as_name().ok()),
                Some(b"Normal" | b"Compatible")
            )
        {
            summary.blend_modes.push(RuleFailure {
                object_id: reported_id,
                description: "used ExtGState dictionary has /BM other than /Normal or /Compatible"
                    .to_owned(),
            });
        }
        inspect_alpha(
            dictionary,
            b"CA",
            reported_id,
            "stroke",
            &mut summary.stroke_alpha,
        );
        inspect_alpha(
            dictionary,
            b"ca",
            reported_id,
            "fill",
            &mut summary.fill_alpha,
        );
        inspect_rendering_intent(dictionary, reported_id, "used ExtGState", &mut summary);
    }

    for object_id in &content.used_xobject_ids {
        let Some(stream) = document
            .objects
            .get(object_id)
            .and_then(|object| object.as_stream().ok())
        else {
            continue;
        };
        let reported_id = Some((*object_id).into());
        if stream.dict.has(b"SMask") {
            summary.xobject_soft_masks.push(RuleFailure {
                object_id: reported_id,
                description: "invoked XObject dictionary contains /SMask".to_owned(),
            });
        }
        if stream
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|value| value.as_name().ok())
            == Some(b"Form".as_slice())
        {
            inspect_group(
                document,
                &stream.dict,
                reported_id,
                "invoked Form",
                limits,
                &mut summary,
            )?;
        }
        if stream
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|value| value.as_name().ok())
            == Some(b"Image".as_slice())
        {
            inspect_rendering_intent(&stream.dict, reported_id, "invoked image", &mut summary);
        }
    }

    for (_, page_id) in document.get_pages() {
        let Some(page) = document
            .objects
            .get(&page_id)
            .and_then(|object| object.as_dict().ok())
        else {
            continue;
        };
        inspect_group(
            document,
            page,
            Some(page_id.into()),
            "page",
            limits,
            &mut summary,
        )?;
    }
    Ok(summary)
}

fn inspect_alpha(
    dictionary: &Dictionary,
    key: &[u8],
    object_id: Option<PdfObjectId>,
    kind: &str,
    failures: &mut Vec<RuleFailure>,
) {
    let Some(value) = dictionary
        .get(key)
        .ok()
        .and_then(|value| value.as_float().ok())
    else {
        return;
    };
    if (value - 1.0).abs() >= 0.000_001 {
        failures.push(RuleFailure {
            object_id,
            description: format!("used ExtGState dictionary has {kind} alpha {value} instead of 1"),
        });
    }
}

fn inspect_group(
    document: &Document,
    owner: &Dictionary,
    object_id: Option<PdfObjectId>,
    context: &str,
    limits: &SafetyLimits,
    summary: &mut GraphicsSummary,
) -> Result<(), PdfError> {
    let Ok(group) = owner.get(b"Group") else {
        return Ok(());
    };
    let Some(group) = resolve_optional(document, group, limits.max_reference_depth)?
        .and_then(|object| object.as_dict().ok())
    else {
        return Ok(());
    };
    if group.get(b"S").ok().and_then(|value| value.as_name().ok())
        == Some(b"Transparency".as_slice())
    {
        summary.transparency_groups.push(RuleFailure {
            object_id,
            description: format!("{context} contains a transparency group"),
        });
    }
    Ok(())
}

pub(crate) fn is_standard_rendering_intent(name: &str) -> bool {
    matches!(
        name,
        "RelativeColorimetric" | "AbsoluteColorimetric" | "Perceptual" | "Saturation"
    )
}

fn inspect_rendering_intent(
    dictionary: &Dictionary,
    object_id: Option<PdfObjectId>,
    context: &str,
    summary: &mut GraphicsSummary,
) {
    let Some(name) = dictionary
        .get(b"RI")
        .or_else(|_| dictionary.get(b"Intent"))
        .ok()
        .and_then(|value| value.as_name().ok())
    else {
        return;
    };
    let name = String::from_utf8_lossy(name);
    if !is_standard_rendering_intent(name.as_ref()) {
        summary.rendering_intents.push(RuleFailure {
            object_id,
            description: format!("{context} uses rendering intent /{name}"),
        });
    }
}

fn resolve_dictionary<'a>(
    document: &'a Document,
    object_id: ObjectId,
    limits: &SafetyLimits,
) -> Result<Option<&'a Dictionary>, PdfError> {
    let Some(object) = document.objects.get(&object_id) else {
        return Ok(None);
    };
    Ok(resolve_optional(document, object, limits.max_reference_depth)?.and_then(dictionary_based))
}
