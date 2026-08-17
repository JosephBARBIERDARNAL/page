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
    pub(crate) transparency_groups_missing_cs: Vec<RuleFailure>,
    pub(crate) pages_with_transparency_missing_cs: Vec<RuleFailure>,
    pub(crate) blend_modes: Vec<RuleFailure>,
    pub(crate) blend_modes_pdfa2: Vec<RuleFailure>,
    pub(crate) stroke_alpha: Vec<RuleFailure>,
    pub(crate) fill_alpha: Vec<RuleFailure>,
    pub(crate) extgstate_htp: Vec<RuleFailure>,
    pub(crate) halftone_types: Vec<RuleFailure>,
    pub(crate) halftone_names: Vec<RuleFailure>,
    pub(crate) halftone_transfer_functions: Vec<RuleFailure>,
}

pub(crate) fn inspect(
    document: &Document,
    content: &ContentExecutionSummary,
    pages: &[PageEntry],
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
            inspect_extgstate_halftone(
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

    for (index, page_entry) in pages.iter().enumerate() {
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
        let page_number = index + 1;
        if content
            .pages_with_transparency
            .contains(&(page_number as u32))
            && !page_has_group_cs(document, page, limits)?
        {
            summary
                .pages_with_transparency_missing_cs
                .push(RuleFailure {
                    object_id: page_entry.object_id().map(Into::into),
                    description:
                        "page contains transparency without a /Group /CS blending colour space"
                            .to_owned(),
                });
        }
    }
    inspect_all_halftones_and_extgstates(document, &mut summary);
    Ok(summary)
}

fn page_has_group_cs(
    document: &Document,
    page: &Dictionary,
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    let Some(group) = page
        .get(b"Group")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|value| value.as_dict().ok())
    else {
        return Ok(false);
    };
    Ok(
        resolved_name(document, group, b"S", limits.max_reference_depth)?
            == Some(b"Transparency".as_slice())
            && contains_key(group, b"CS"),
    )
}

fn inspect_all_halftones_and_extgstates(document: &Document, summary: &mut GraphicsSummary) {
    for (object_id, object) in &document.objects {
        let Some(dictionary) = object.as_dict().ok() else {
            continue;
        };
        let object_id = Some((*object_id).into());
        if contains_key(dictionary, b"HTP") {
            summary.extgstate_htp.push(RuleFailure {
                object_id,
                description: "an ExtGState dictionary contains /HTP".to_owned(),
            });
        }
        let Some(halftone_type) = dictionary
            .get(b"HalftoneType")
            .ok()
            .and_then(|value| value.as_i64().ok())
        else {
            continue;
        };
        if !matches!(halftone_type, 1 | 5) {
            summary.halftone_types.push(RuleFailure {
                object_id,
                description: format!(
                    "a halftone dictionary has /HalftoneType {halftone_type} instead of 1 or 5"
                ),
            });
        }
        if contains_key(dictionary, b"HalftoneName") {
            summary.halftone_names.push(RuleFailure {
                object_id,
                description: "a halftone dictionary contains /HalftoneName".to_owned(),
            });
        }
    }
}

fn inspect_extgstate_halftone(
    document: &Document,
    extgstate: &Dictionary,
    object_id: Option<PdfObjectId>,
    limits: &SafetyLimits,
    summary: &mut GraphicsSummary,
) -> Result<(), PdfError> {
    let Some(halftone) = extgstate
        .get(b"HT")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(dictionary_based)
    else {
        return Ok(());
    };
    inspect_halftone_dictionary(document, halftone, object_id, limits, summary)
}

fn inspect_halftone_dictionary(
    document: &Document,
    halftone: &Dictionary,
    object_id: Option<PdfObjectId>,
    limits: &SafetyLimits,
    summary: &mut GraphicsSummary,
) -> Result<(), PdfError> {
    if let Some(halftone_type) = halftone
        .get(b"HalftoneType")
        .ok()
        .and_then(|value| value.as_i64().ok())
        && !matches!(halftone_type, 1 | 5)
    {
        summary.halftone_types.push(RuleFailure {
            object_id,
            description: format!(
                "a halftone dictionary has /HalftoneType {halftone_type} instead of 1 or 5"
            ),
        });
    }
    if contains_key(halftone, b"HalftoneName") {
        summary.halftone_names.push(RuleFailure {
            object_id,
            description: "a halftone dictionary contains /HalftoneName".to_owned(),
        });
    }
    inspect_halftone_transfer_function(document, halftone, object_id, None, limits, summary)?;
    if halftone
        .get(b"HalftoneType")
        .ok()
        .and_then(|value| value.as_i64().ok())
        == Some(5)
    {
        for (colorant_name, value) in halftone.iter() {
            let Some(child) = resolve_optional(document, value, limits.max_reference_depth)?
                .and_then(dictionary_based)
            else {
                continue;
            };
            inspect_halftone_transfer_function(
                document,
                child,
                object_id,
                Some(colorant_name),
                limits,
                summary,
            )?;
        }
    }
    Ok(())
}

fn inspect_halftone_transfer_function(
    document: &Document,
    halftone: &Dictionary,
    object_id: Option<PdfObjectId>,
    colorant_name: Option<&[u8]>,
    limits: &SafetyLimits,
    summary: &mut GraphicsSummary,
) -> Result<(), PdfError> {
    let has_transfer_function = halftone
        .get(b"TransferFunction")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .is_some_and(|value| !matches!(value, lopdf::Object::Null));
    let valid = match colorant_name {
        Some(b"Default") => true,
        None | Some(b"Cyan" | b"Magenta" | b"Yellow" | b"Black") => !has_transfer_function,
        Some(_) => has_transfer_function,
    };
    if !valid {
        let context = colorant_name
            .map(|name| format!(" /{}", String::from_utf8_lossy(name)))
            .unwrap_or_else(|| " root".to_owned());
        summary.halftone_transfer_functions.push(RuleFailure {
            object_id,
            description: format!("a{context} halftone has an invalid /TransferFunction entry"),
        });
    }
    Ok(())
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
    if contains_key(dictionary, b"HTP") {
        summary.extgstate_htp.push(RuleFailure {
            object_id,
            description: "a used ExtGState dictionary contains /HTP".to_owned(),
        });
    }
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
        && let Some(blend_mode) =
            resolved_name(document, dictionary, b"BM", limits.max_reference_depth)?
    {
        if !matches!(blend_mode, b"Normal" | b"Compatible") {
            summary.blend_modes.push(RuleFailure {
                object_id,
                description: format!(
                    "used ExtGState dictionary has /BM /{} outside PDF/A-1's allowed set",
                    String::from_utf8_lossy(blend_mode)
                ),
            });
        }
        if !matches!(
            blend_mode,
            b"Normal"
                | b"Compatible"
                | b"Multiply"
                | b"Screen"
                | b"Overlay"
                | b"Darken"
                | b"Lighten"
                | b"ColorDodge"
                | b"ColorBurn"
                | b"HardLight"
                | b"SoftLight"
                | b"Difference"
                | b"Exclusion"
                | b"Hue"
                | b"Saturation"
                | b"Color"
                | b"Luminosity"
        ) {
            summary.blend_modes_pdfa2.push(RuleFailure {
                object_id,
                description: format!(
                    "used ExtGState dictionary has unsupported blend mode /{}",
                    String::from_utf8_lossy(blend_mode)
                ),
            });
        }
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
