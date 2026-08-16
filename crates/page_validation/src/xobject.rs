use std::collections::BTreeMap;

use lopdf::{Document, Object};

use crate::content_support::{ContentExecutionSummary, XObjectUseKind};
use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::object_resolution::{
    ResourceKey, contains_key, dictionary_based, resolve_optional, resolved_bool, resolved_integer,
    resolved_name,
};
use crate::report::RuleFailure;

#[derive(Clone, Debug, Default)]
pub(crate) struct XObjectSummary {
    pub(crate) image_alternates: Vec<RuleFailure>,
    pub(crate) xobject_opi: Vec<RuleFailure>,
    pub(crate) image_interpolate: Vec<RuleFailure>,
    pub(crate) image_bits_per_component: Vec<RuleFailure>,
    pub(crate) image_bits_per_component_pdfa2: Vec<RuleFailure>,
    pub(crate) mask_bits_per_component: Vec<RuleFailure>,
    pub(crate) form_postscript: Vec<RuleFailure>,
    pub(crate) form_reference: Vec<RuleFailure>,
    pub(crate) postscript_xobject: Vec<RuleFailure>,
}

pub(crate) fn inspect(
    document: &Document,
    execution: &ContentExecutionSummary,
    limits: &SafetyLimits,
) -> Result<XObjectSummary, PdfError> {
    let mut summary = XObjectSummary::default();
    let mut uses = BTreeMap::<ResourceKey, (&Object, bool, bool, bool)>::new();
    for use_ in &execution.xobjects {
        let entry = uses
            .entry(use_.key.clone())
            .or_insert((&use_.object, false, false, false));
        match use_.kind {
            XObjectUseKind::Appearance => entry.3 = true,
            XObjectUseKind::ExplicitMask => entry.2 = true,
            _ => entry.1 = true,
        }
    }
    for (key, (object, is_ordinary_image, is_explicit_mask, is_appearance)) in uses {
        let Some(dictionary) = dictionary_based(object) else {
            continue;
        };
        let object_id = key.object_id();
        let subtype = resolved_name(document, dictionary, b"Subtype", limits.max_reference_depth)?;
        if subtype.is_some() || is_appearance {
            let kind = if is_appearance {
                "appearance Form"
            } else {
                "XObject"
            };
            inspect_common_xobject(dictionary, object_id, kind, &mut summary);
        }
        if subtype == Some(b"Image".as_slice()) && (is_ordinary_image || is_explicit_mask) {
            inspect_image(
                document,
                dictionary,
                object_id,
                is_ordinary_image,
                is_explicit_mask,
                limits,
                &mut summary,
            )?;
        }
        if (subtype == Some(b"Form".as_slice()) && (is_ordinary_image || is_explicit_mask))
            || is_appearance
        {
            inspect_form(document, dictionary, object_id, limits, &mut summary)?;
        }
        if subtype == Some(b"PS".as_slice()) && (is_ordinary_image || is_explicit_mask) {
            summary.postscript_xobject.push(RuleFailure {
                object_id,
                description: "XObject has /Subtype /PS".to_owned(),
            });
        }
    }
    Ok(summary)
}

fn inspect_common_xobject(
    dictionary: &lopdf::Dictionary,
    object_id: Option<PdfObjectId>,
    kind: &str,
    summary: &mut XObjectSummary,
) {
    if contains_key(dictionary, b"OPI") {
        summary.xobject_opi.push(RuleFailure {
            object_id,
            description: format!("{kind} dictionary contains /OPI"),
        });
    }
}

fn inspect_image(
    document: &Document,
    dictionary: &lopdf::Dictionary,
    object_id: Option<PdfObjectId>,
    is_ordinary_image: bool,
    is_explicit_mask: bool,
    limits: &SafetyLimits,
    summary: &mut XObjectSummary,
) -> Result<(), PdfError> {
    if contains_key(dictionary, b"Alternates") {
        summary.image_alternates.push(RuleFailure {
            object_id,
            description: "image dictionary contains /Alternates".to_owned(),
        });
    }
    if contains_key(dictionary, b"Interpolate")
        && resolved_bool(
            document,
            dictionary,
            b"Interpolate",
            limits.max_reference_depth,
        )? != Some(false)
    {
        summary.image_interpolate.push(RuleFailure {
            object_id,
            description: "image dictionary /Interpolate is not false".to_owned(),
        });
    }

    let bits_per_component = resolved_integer(
        document,
        dictionary,
        b"BitsPerComponent",
        limits.max_reference_depth,
    )?;
    // veraPDF 1.30.2's `isMask` getter reads only a direct boolean. An
    // indirect `true` is therefore modeled as an ordinary image.
    let is_stencil_mask = dictionary
        .get(b"ImageMask")
        .ok()
        .and_then(|value| value.as_bool().ok())
        == Some(true);
    if is_explicit_mask && bits_per_component.is_some_and(|value| value != 1) {
        summary.mask_bits_per_component.push(RuleFailure {
            object_id,
            description: format!(
                "image mask dictionary has /BitsPerComponent {bits_per_component:?}"
            ),
        });
    }
    if is_ordinary_image
        && !is_stencil_mask
        && bits_per_component.is_some_and(|value| !matches!(value, 1 | 2 | 4 | 8))
    {
        summary.image_bits_per_component.push(RuleFailure {
            object_id,
            description: format!("image dictionary has /BitsPerComponent {bits_per_component:?}"),
        });
    }
    if is_ordinary_image
        && !is_stencil_mask
        && bits_per_component.is_some_and(|value| !matches!(value, 1 | 2 | 4 | 8 | 16))
    {
        summary.image_bits_per_component_pdfa2.push(RuleFailure {
            object_id,
            description: format!("image dictionary has /BitsPerComponent {bits_per_component:?}"),
        });
    }
    Ok(())
}

fn inspect_form(
    document: &Document,
    dictionary: &lopdf::Dictionary,
    object_id: Option<PdfObjectId>,
    limits: &SafetyLimits,
    summary: &mut XObjectSummary,
) -> Result<(), PdfError> {
    let subtype2_is_ps = resolved_name(
        document,
        dictionary,
        b"Subtype2",
        limits.max_reference_depth,
    )? == Some(b"PS".as_slice());
    let contains_modeled_ps = match dictionary.get(b"PS") {
        Ok(value) => resolve_optional(document, value, limits.max_reference_depth)?
            .is_some_and(|object| matches!(object, Object::Stream(_))),
        Err(_) => false,
    };
    if subtype2_is_ps || contains_modeled_ps {
        summary.form_postscript.push(RuleFailure {
            object_id,
            description: "Form dictionary contains /PS or /Subtype2 /PS".to_owned(),
        });
    }
    if contains_key(dictionary, b"Ref") {
        summary.form_reference.push(RuleFailure {
            object_id,
            description: "Form dictionary contains /Ref".to_owned(),
        });
    }
    Ok(())
}
