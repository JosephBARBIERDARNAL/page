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
    pub(crate) image_opi: Vec<RuleFailure>,
    pub(crate) image_interpolate: Vec<RuleFailure>,
    pub(crate) image_bits_per_component: Vec<RuleFailure>,
    pub(crate) image_bits_per_component_pdfa2: Vec<RuleFailure>,
    pub(crate) jpeg2000_failures: [Vec<RuleFailure>; 5],
    pub(crate) mask_bits_per_component: Vec<RuleFailure>,
    pub(crate) form_opi: Vec<RuleFailure>,
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
        if subtype == Some(b"Image".as_slice()) && (is_ordinary_image || is_explicit_mask) {
            inspect_image(
                document,
                dictionary,
                object,
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

fn inspect_image(
    document: &Document,
    dictionary: &lopdf::Dictionary,
    object: &Object,
    object_id: Option<PdfObjectId>,
    is_ordinary_image: bool,
    is_explicit_mask: bool,
    limits: &SafetyLimits,
    summary: &mut XObjectSummary,
) -> Result<(), PdfError> {
    if is_ordinary_image
        && has_jpx_filter(dictionary)
        && let Ok(stream) = object.as_stream()
    {
        inspect_jpeg2000(stream, object_id, limits, summary)?;
    }
    if contains_key(dictionary, b"OPI") {
        summary.image_opi.push(RuleFailure {
            object_id,
            description: "image dictionary contains /OPI".to_owned(),
        });
    }
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

fn has_jpx_filter(dictionary: &lopdf::Dictionary) -> bool {
    let Ok(filter) = dictionary.get(b"Filter") else {
        return false;
    };
    match filter {
        Object::Name(name) => name == b"JPXDecode",
        Object::Array(filters) => filters
            .iter()
            .any(|filter| filter.as_name().ok() == Some(b"JPXDecode")),
        _ => false,
    }
}

fn inspect_jpeg2000(
    stream: &lopdf::Stream,
    object_id: Option<PdfObjectId>,
    limits: &SafetyLimits,
    summary: &mut XObjectSummary,
) -> Result<(), PdfError> {
    let bytes = stream
        .decompressed_content_with_limit(limits.max_decoded_stream_size)
        .map_err(|_| PdfError::ContentDecodeLimit(limits.max_decoded_stream_size))?;
    let mut channels = None;
    let mut depths = Vec::new();
    let mut methods = Vec::new();
    let mut approx_one = 0usize;
    let mut enum_cs = None;
    if bytes.starts_with(&[0xff, 0x4f]) {
        if let Some((csiz, ssiz)) = parse_j2k_siz(&bytes) {
            channels = Some(csiz);
            depths = ssiz;
        }
    } else {
        parse_jp2_boxes(
            &bytes,
            &mut channels,
            &mut depths,
            &mut methods,
            &mut approx_one,
            &mut enum_cs,
        );
    }
    if let Some(channels) = channels
        && !matches!(channels, 1 | 3 | 4)
    {
        summary.jpeg2000_failures[0].push(jpx_failure(
            object_id,
            format!("has {channels} colour channels"),
        ));
    }
    if methods.len() > 1 && approx_one != 1 {
        summary.jpeg2000_failures[1].push(jpx_failure(
            object_id,
            "has an invalid number of APPROX=1 colour specifications",
        ));
    }
    if methods.iter().any(|method| !matches!(method, 1..=3)) {
        summary.jpeg2000_failures[2].push(jpx_failure(
            object_id,
            "has an invalid JPEG2000 colour method",
        ));
    }
    if enum_cs == Some(19) {
        summary.jpeg2000_failures[3].push(jpx_failure(
            object_id,
            "uses enumerated colour space 19 (CIEJab)",
        ));
    }
    if !depths.is_empty()
        && (depths.iter().any(|depth| !(1..=38).contains(depth))
            || depths.windows(2).any(|pair| pair[0] != pair[1]))
    {
        summary.jpeg2000_failures[4].push(jpx_failure(
            object_id,
            "has an invalid or inconsistent bit depth",
        ));
    }
    Ok(())
}

fn jpx_failure(object_id: Option<PdfObjectId>, description: impl Into<String>) -> RuleFailure {
    RuleFailure {
        object_id,
        description: description.into(),
    }
}

fn parse_j2k_siz(bytes: &[u8]) -> Option<(usize, Vec<usize>)> {
    let marker = bytes.windows(2).position(|pair| pair == [0xff, 0x51])?;
    let length = usize::from(u16::from_be_bytes([
        *bytes.get(marker + 2)?,
        *bytes.get(marker + 3)?,
    ]));
    let end = marker.checked_add(2 + length)?;
    let segment = bytes.get(marker + 4..end)?;
    let csiz = usize::from(u16::from_be_bytes([*segment.get(34)?, *segment.get(35)?]));
    let mut depths = Vec::with_capacity(csiz);
    for index in 0..csiz {
        depths.push(usize::from(segment.get(36 + index * 3)? & 0x7f) + 1);
    }
    Some((csiz, depths))
}

fn parse_jp2_boxes(
    bytes: &[u8],
    channels: &mut Option<usize>,
    depths: &mut Vec<usize>,
    methods: &mut Vec<u8>,
    approx_one: &mut usize,
    enum_cs: &mut Option<u32>,
) {
    let mut position = 0usize;
    while position + 8 <= bytes.len() {
        let length = u32::from_be_bytes(bytes[position..position + 4].try_into().unwrap()) as usize;
        if length < 8 || position + length > bytes.len() {
            break;
        }
        let kind = &bytes[position + 4..position + 8];
        let payload = &bytes[position + 8..position + length];
        if kind == b"ihdr" && payload.len() >= 11 {
            *channels = Some(usize::from(u16::from_be_bytes([payload[8], payload[9]])));
            depths.push(usize::from((payload[10] & 0x7f) + 1));
        } else if kind == b"colr" && payload.len() >= 3 {
            methods.push(payload[0]);
            if payload[2] == 1 {
                *approx_one += 1;
            }
            if payload[0] == 1 && payload.len() >= 7 {
                *enum_cs = Some(u32::from_be_bytes(payload[3..7].try_into().unwrap()));
            }
        } else if kind == b"jp2h" || kind == b"jp2c" {
            parse_jp2_boxes(payload, channels, depths, methods, approx_one, enum_cs);
        }
        position += length;
    }
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
    if contains_key(dictionary, b"OPI") {
        summary.form_opi.push(RuleFailure {
            object_id,
            description: "Form dictionary contains /OPI".to_owned(),
        });
    }
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

#[cfg(test)]
mod tests {
    use super::parse_j2k_siz;

    #[test]
    fn parses_jpeg2000_siz_channels_and_depths() {
        let mut bytes = vec![0xff, 0x4f, 0xff, 0x51, 0, 45];
        bytes.extend_from_slice(&[0; 36]);
        bytes[6 + 34] = 0;
        bytes[6 + 35] = 3;
        bytes.extend_from_slice(&[7, 1, 1, 7, 1, 1, 7, 1, 1]);
        assert_eq!(parse_j2k_siz(&bytes), Some((3, vec![8, 8, 8])));
    }
}
