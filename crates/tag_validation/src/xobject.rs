use std::collections::BTreeMap;

use lopdf::{Document, Object};

use crate::content_support::{ContentExecutionSummary, XObjectUseKind};
use crate::model::PdfObjectId;
use crate::object_resolution::{ResourceKey, contains_key};
use crate::report::RuleFailure;

#[derive(Clone, Debug, Default)]
pub(crate) struct XObjectSummary {
    pub(crate) image_alternates: Vec<RuleFailure>,
    pub(crate) xobject_opi: Vec<RuleFailure>,
    pub(crate) image_interpolate: Vec<RuleFailure>,
    pub(crate) image_bits_per_component: Vec<RuleFailure>,
    pub(crate) mask_bits_per_component: Vec<RuleFailure>,
    pub(crate) form_postscript: Vec<RuleFailure>,
    pub(crate) form_reference: Vec<RuleFailure>,
    pub(crate) postscript_xobject: Vec<RuleFailure>,
}

pub(crate) fn inspect(document: &Document, execution: &ContentExecutionSummary) -> XObjectSummary {
    let mut summary = XObjectSummary::default();
    let mut uses = BTreeMap::<ResourceKey, (&Object, bool)>::new();
    for use_ in &execution.xobjects {
        let entry = uses
            .entry(use_.key.clone())
            .or_insert((&use_.object, false));
        entry.1 |= use_.kind == XObjectUseKind::ExplicitMask;
    }
    for (key, (object, is_explicit_mask)) in uses {
        let Object::Stream(stream) = object else {
            continue;
        };
        let object_id = key.object_id();
        let subtype = stream
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|value| value.as_name().ok());
        match subtype {
            Some(b"Image") => {
                inspect_common_xobject(&stream.dict, object_id, "image", &mut summary);
                inspect_image(&stream.dict, object_id, is_explicit_mask, &mut summary);
            }
            Some(b"Form") => {
                inspect_common_xobject(&stream.dict, object_id, "Form", &mut summary);
                inspect_form(document, &stream.dict, object_id, &mut summary);
            }
            Some(b"PS") => {
                inspect_common_xobject(&stream.dict, object_id, "PostScript XObject", &mut summary);
                summary.postscript_xobject.push(RuleFailure {
                    object_id,
                    description: "XObject has /Subtype /PS".to_owned(),
                });
            }
            _ => {}
        }
    }
    summary
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
    dictionary: &lopdf::Dictionary,
    object_id: Option<PdfObjectId>,
    is_explicit_mask: bool,
    summary: &mut XObjectSummary,
) {
    if contains_key(dictionary, b"Alternates") {
        summary.image_alternates.push(RuleFailure {
            object_id,
            description: "image dictionary contains /Alternates".to_owned(),
        });
    }
    if contains_key(dictionary, b"Interpolate")
        && dictionary
            .get(b"Interpolate")
            .ok()
            .and_then(|value| value.as_bool().ok())
            != Some(false)
    {
        summary.image_interpolate.push(RuleFailure {
            object_id,
            description: "image dictionary /Interpolate is not false".to_owned(),
        });
    }

    let bits_per_component = dictionary
        .get(b"BitsPerComponent")
        .ok()
        .and_then(|value| value.as_i64().ok());
    let is_stencil_mask = dictionary
        .get(b"ImageMask")
        .ok()
        .and_then(|value| value.as_bool().ok())
        == Some(true);
    if is_explicit_mask {
        if bits_per_component.is_some_and(|value| value != 1) {
            summary.mask_bits_per_component.push(RuleFailure {
                object_id,
                description: format!(
                    "image mask dictionary has /BitsPerComponent {bits_per_component:?}"
                ),
            });
        }
    } else if !is_stencil_mask
        && bits_per_component.is_some_and(|value| !matches!(value, 1 | 2 | 4 | 8))
    {
        summary.image_bits_per_component.push(RuleFailure {
            object_id,
            description: format!("image dictionary has /BitsPerComponent {bits_per_component:?}"),
        });
    }
}

fn inspect_form(
    document: &Document,
    dictionary: &lopdf::Dictionary,
    object_id: Option<PdfObjectId>,
    summary: &mut XObjectSummary,
) {
    let subtype2_is_ps = dictionary
        .get(b"Subtype2")
        .ok()
        .and_then(|value| value.as_name().ok())
        == Some(b"PS".as_slice());
    let contains_modeled_ps = dictionary.get(b"PS").ok().is_some_and(|value| match value {
        Object::Stream(_) => true,
        Object::Reference(id) => document
            .objects
            .get(id)
            .is_some_and(|object| matches!(object, Object::Stream(_))),
        _ => false,
    });
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
}
