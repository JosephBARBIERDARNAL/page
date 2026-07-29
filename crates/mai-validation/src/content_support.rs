use std::collections::BTreeSet;

use lopdf::{Dictionary, Document, Object, Stream};

use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::object_resolution::resolve_optional;

pub(crate) fn decode_content_stream(
    stream: &Stream,
    limits: &SafetyLimits,
    decoded_bytes: &mut usize,
) -> Result<Vec<u8>, PdfError> {
    let remaining = limits
        .max_decoded_stream_size
        .saturating_sub(*decoded_bytes);
    let bytes = match stream.decompressed_content_with_limit(remaining) {
        Ok(bytes) => bytes,
        Err(lopdf::Error::Decompress(lopdf::DecompressError::MemoryLimitExceeded { .. })) => {
            return Err(PdfError::ContentDecodeLimit(limits.max_decoded_stream_size));
        }
        Err(_) if stream.content.len() <= remaining => stream.content.clone(),
        Err(_) => return Err(PdfError::ContentDecodeLimit(limits.max_decoded_stream_size)),
    };
    if bytes.len() > remaining {
        return Err(PdfError::ContentDecodeLimit(limits.max_decoded_stream_size));
    }
    *decoded_bytes = decoded_bytes.saturating_add(bytes.len());
    Ok(bytes)
}

pub(crate) fn inherited_page_resources<'a>(
    document: &'a Document,
    mut node: &'a Dictionary,
    limits: &SafetyLimits,
) -> Result<Option<&'a Dictionary>, PdfError> {
    let mut visited = BTreeSet::new();
    for _ in 0..=limits.max_reference_depth {
        if let Ok(resources) = node.get(b"Resources") {
            return Ok(
                resolve_optional(document, resources, limits.max_reference_depth)?
                    .and_then(|object| object.as_dict().ok()),
            );
        }
        let Ok(parent) = node.get(b"Parent") else {
            return Ok(None);
        };
        if let Object::Reference(id) = parent
            && !visited.insert(*id)
        {
            return Err(PdfError::ReferenceDepth(limits.max_reference_depth));
        }
        let Some(parent) = resolve_optional(document, parent, limits.max_reference_depth)? else {
            return Ok(None);
        };
        let Ok(parent) = parent.as_dict() else {
            return Ok(None);
        };
        node = parent;
    }
    Err(PdfError::ReferenceDepth(limits.max_reference_depth))
}

pub(crate) fn resource_once<'a>(
    document: &'a Document,
    limits: &SafetyLimits,
    resources: Option<&'a Dictionary>,
    category: &[u8],
    name: &[u8],
) -> Result<Option<&'a Object>, PdfError> {
    let Some(resources) = resources else {
        return Ok(None);
    };
    let Ok(category) = resources.get(category) else {
        return Ok(None);
    };
    let Some(category) = resolve_optional(document, category, limits.max_reference_depth)? else {
        return Ok(None);
    };
    let Ok(category) = category.as_dict() else {
        return Ok(None);
    };
    Ok(category.get(name).ok())
}
