use std::collections::{BTreeMap, BTreeSet};

use lopdf::{Dictionary, Document};

use crate::content_support::ContentExecutionSummary;
use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::object_resolution::{contains_key, dictionary_based, resolve_optional, resolved_name};
use crate::page_tree::PageEntry;
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
    content: &ContentExecutionSummary,
    pages: &BTreeMap<u32, PageEntry>,
    limits: &SafetyLimits,
) -> Result<GraphicsSummary, PdfError> {
    let mut summary = GraphicsSummary::default();
    for (name, context) in &content.invalid_rendering_intents {
        summary.rendering_intents.push(RuleFailure {
            object_id: None,
            description: format!("content in {context} uses rendering intent /{name}"),
        });
    }

    let mut extgstates = BTreeSet::new();
    for use_ in &content.extgstates {
        if extgstates.insert(use_.key.clone()) {
            inspect_extgstate(
                document,
                &use_.dictionary,
                use_.key.object_id(),
                limits,
                &mut summary,
            )?;
        }
    }

    let mut xobjects = BTreeMap::new();
    for use_ in &content.xobjects {
        let entry = xobjects
            .entry(use_.key.clone())
            .or_insert((&use_.object, false, false, false));
        match use_.kind {
            crate::content_support::XObjectUseKind::Appearance => entry.1 = true,
            crate::content_support::XObjectUseKind::ExplicitMask => entry.2 = true,
            crate::content_support::XObjectUseKind::Painted
            | crate::content_support::XObjectUseKind::Alternate
            | crate::content_support::XObjectUseKind::SoftMask => {
                entry.2 = true;
                entry.3 = true;
            }
        }
    }
    for (key, (object, is_appearance, has_declared_xobject_role, has_image_intent_role)) in xobjects
    {
        let Some(dictionary) = dictionary_based(object) else {
            continue;
        };
        let reported_id = key.object_id();
        if contains_key(dictionary, b"SMask") {
            summary.xobject_soft_masks.push(RuleFailure {
                object_id: reported_id,
                description: "invoked XObject dictionary contains /SMask".to_owned(),
            });
        }
        let subtype = resolved_name(document, dictionary, b"Subtype", limits.max_reference_depth)?;
        if (subtype == Some(b"Form".as_slice()) && has_declared_xobject_role) || is_appearance {
            inspect_group(
                document,
                dictionary,
                reported_id,
                "invoked Form",
                limits,
                &mut summary,
            )?;
        }
        if subtype == Some(b"Image".as_slice()) && has_image_intent_role {
            inspect_rendering_intent(
                document,
                dictionary,
                reported_id,
                "invoked image",
                limits,
                &mut summary,
            )?;
        }
    }

    for page_entry in pages.values() {
        let Some(page) = page_entry.resolve(document) else {
            continue;
        };
        inspect_group(
            document,
            page,
            page_entry.object_id().map(Into::into),
            "page",
            limits,
            &mut summary,
        )?;
    }
    Ok(summary)
}

fn inspect_alpha(
    document: &Document,
    dictionary: &Dictionary,
    key: &[u8],
    object_id: Option<PdfObjectId>,
    kind: &str,
    limits: &SafetyLimits,
    failures: &mut Vec<RuleFailure>,
) -> Result<(), PdfError> {
    let Some(value) = dictionary
        .get(key)
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|value| value.as_float().ok())
    else {
        return Ok(());
    };
    if (value - 1.0).abs() >= 0.000_001 {
        failures.push(RuleFailure {
            object_id,
            description: format!("used ExtGState dictionary has {kind} alpha {value} instead of 1"),
        });
    }
    Ok(())
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
    if resolved_name(document, group, b"S", limits.max_reference_depth)?
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
    document: &Document,
    dictionary: &Dictionary,
    object_id: Option<PdfObjectId>,
    context: &str,
    limits: &SafetyLimits,
    summary: &mut GraphicsSummary,
) -> Result<(), PdfError> {
    let key = if dictionary.has(b"RI") {
        b"RI".as_slice()
    } else {
        b"Intent".as_slice()
    };
    let Some(name) = resolved_name(document, dictionary, key, limits.max_reference_depth)? else {
        return Ok(());
    };
    let name = String::from_utf8_lossy(name);
    if !is_standard_rendering_intent(name.as_ref()) {
        summary.rendering_intents.push(RuleFailure {
            object_id,
            description: format!("{context} uses rendering intent /{name}"),
        });
    }
    Ok(())
}

fn inspect_extgstate(
    document: &Document,
    dictionary: &Dictionary,
    object_id: Option<PdfObjectId>,
    limits: &SafetyLimits,
    summary: &mut GraphicsSummary,
) -> Result<(), PdfError> {
    if contains_key(dictionary, b"TR") {
        summary.transfer_functions.push(RuleFailure {
            object_id,
            description: "used ExtGState dictionary contains /TR".to_owned(),
        });
    }
    if contains_key(dictionary, b"TR2")
        && resolved_name(document, dictionary, b"TR2", limits.max_reference_depth)?
            != Some(b"Default".as_slice())
    {
        summary.transfer_functions_2.push(RuleFailure {
            object_id,
            description: "used ExtGState dictionary has /TR2 other than /Default".to_owned(),
        });
    }
    if contains_key(dictionary, b"SMask")
        && resolved_name(document, dictionary, b"SMask", limits.max_reference_depth)?
            != Some(b"None".as_slice())
    {
        summary.extgstate_soft_masks.push(RuleFailure {
            object_id,
            description: "used ExtGState dictionary has /SMask other than /None".to_owned(),
        });
    }
    if contains_key(dictionary, b"BM")
        && !matches!(
            resolved_name(document, dictionary, b"BM", limits.max_reference_depth)?,
            Some(b"Normal" | b"Compatible")
        )
    {
        summary.blend_modes.push(RuleFailure {
            object_id,
            description: "used ExtGState dictionary has /BM other than /Normal or /Compatible"
                .to_owned(),
        });
    }
    inspect_alpha(
        document,
        dictionary,
        b"CA",
        object_id,
        "stroke",
        limits,
        &mut summary.stroke_alpha,
    )?;
    inspect_alpha(
        document,
        dictionary,
        b"ca",
        object_id,
        "fill",
        limits,
        &mut summary.fill_alpha,
    )?;
    inspect_rendering_intent(
        document,
        dictionary,
        object_id,
        "used ExtGState",
        limits,
        summary,
    )?;
    Ok(())
}
