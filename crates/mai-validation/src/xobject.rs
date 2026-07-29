use std::collections::BTreeSet;

use lopdf::{Document, Object, ObjectId};

use crate::model::PdfObjectId;

#[derive(Clone, Debug, Default)]
pub(crate) struct XObjectSummary {
    pub(crate) image_alternates: Vec<XObjectFailure>,
    pub(crate) xobject_opi: Vec<XObjectFailure>,
    pub(crate) image_interpolate: Vec<XObjectFailure>,
    pub(crate) image_bits_per_component: Vec<XObjectFailure>,
    pub(crate) mask_bits_per_component: Vec<XObjectFailure>,
    pub(crate) form_postscript: Vec<XObjectFailure>,
    pub(crate) form_reference: Vec<XObjectFailure>,
    pub(crate) postscript_xobject: Vec<XObjectFailure>,
}

#[derive(Clone, Debug)]
pub(crate) struct XObjectFailure {
    pub(crate) object_id: PdfObjectId,
    pub(crate) description: String,
}

pub(crate) fn inspect(
    document: &Document,
    used_xobject_ids: &BTreeSet<ObjectId>,
) -> XObjectSummary {
    let mut summary = XObjectSummary::default();
    let explicit_mask_ids = explicit_mask_ids(document);
    for (object_id, object) in &document.objects {
        if !used_xobject_ids.contains(object_id) {
            continue;
        }
        let Object::Stream(stream) = object else {
            continue;
        };
        let subtype = stream
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|value| value.as_name().ok());
        match subtype {
            Some(b"Image") => {
                inspect_common_xobject(&stream.dict, (*object_id).into(), "image", &mut summary);
                inspect_image(
                    &stream.dict,
                    (*object_id).into(),
                    explicit_mask_ids.contains(object_id),
                    &mut summary,
                );
            }
            Some(b"Form") => {
                inspect_common_xobject(&stream.dict, (*object_id).into(), "Form", &mut summary);
                inspect_form(document, &stream.dict, (*object_id).into(), &mut summary);
            }
            Some(b"PS") => {
                inspect_common_xobject(
                    &stream.dict,
                    (*object_id).into(),
                    "PostScript XObject",
                    &mut summary,
                );
                summary.postscript_xobject.push(XObjectFailure {
                    object_id: (*object_id).into(),
                    description: "XObject has /Subtype /PS".to_owned(),
                });
            }
            _ => {}
        }
    }
    summary
}

fn explicit_mask_ids(document: &Document) -> BTreeSet<ObjectId> {
    document
        .objects
        .values()
        .filter_map(|object| object.as_stream().ok())
        .filter(|stream| {
            stream
                .dict
                .get(b"Subtype")
                .ok()
                .and_then(|value| value.as_name().ok())
                == Some(b"Image".as_slice())
        })
        .filter_map(|stream| {
            stream
                .dict
                .get(b"Mask")
                .ok()
                .and_then(|value| value.as_reference().ok())
        })
        .collect()
}

fn inspect_common_xobject(
    dictionary: &lopdf::Dictionary,
    object_id: PdfObjectId,
    kind: &str,
    summary: &mut XObjectSummary,
) {
    if dictionary.has(b"OPI") {
        summary.xobject_opi.push(XObjectFailure {
            object_id,
            description: format!("{kind} dictionary contains /OPI"),
        });
    }
}

fn inspect_image(
    dictionary: &lopdf::Dictionary,
    object_id: PdfObjectId,
    is_explicit_mask: bool,
    summary: &mut XObjectSummary,
) {
    if dictionary.has(b"Alternates") {
        summary.image_alternates.push(XObjectFailure {
            object_id,
            description: "image dictionary contains /Alternates".to_owned(),
        });
    }
    if dictionary.has(b"Interpolate")
        && dictionary
            .get(b"Interpolate")
            .ok()
            .and_then(|value| value.as_bool().ok())
            != Some(false)
    {
        summary.image_interpolate.push(XObjectFailure {
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
            summary.mask_bits_per_component.push(XObjectFailure {
                object_id,
                description: format!(
                    "image mask dictionary has /BitsPerComponent {bits_per_component:?}"
                ),
            });
        }
    } else if !is_stencil_mask
        && bits_per_component.is_some_and(|value| !matches!(value, 1 | 2 | 4 | 8))
    {
        summary.image_bits_per_component.push(XObjectFailure {
            object_id,
            description: format!("image dictionary has /BitsPerComponent {bits_per_component:?}"),
        });
    }
}

fn inspect_form(
    document: &Document,
    dictionary: &lopdf::Dictionary,
    object_id: PdfObjectId,
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
        summary.form_postscript.push(XObjectFailure {
            object_id,
            description: "Form dictionary contains /PS or /Subtype2 /PS".to_owned(),
        });
    }
    if dictionary.has(b"Ref") {
        summary.form_reference.push(XObjectFailure {
            object_id,
            description: "Form dictionary contains /Ref".to_owned(),
        });
    }
}
