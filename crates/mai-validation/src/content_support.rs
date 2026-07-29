use std::collections::BTreeSet;

use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::object_resolution::{resolve_optional, walk_inherited};

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
    node: &'a Dictionary,
    limits: &SafetyLimits,
) -> Result<Option<&'a Dictionary>, PdfError> {
    walk_inherited(
        document,
        node,
        limits,
        b"Resources",
        |document, value, limits| {
            Ok(
                resolve_optional(document, value, limits.max_reference_depth)?
                    .and_then(|object| object.as_dict().ok()),
            )
        },
    )
}

/// Visits every annotation reached through a page `/Annots` array, resolving
/// the array and each entry while deduplicating by indirect object id via
/// the caller-owned `inspected` set. `visit` receives the resolved
/// (unconverted) annotation object; callers extract a dictionary from it
/// however is appropriate for their check (some accept only a plain
/// dictionary, others also accept a stream-backed one via `dictionary_based`).
pub(crate) fn for_each_page_annotation<'a>(
    document: &'a Document,
    limits: &SafetyLimits,
    inspected: &mut BTreeSet<ObjectId>,
    mut visit: impl FnMut(u32, usize, Option<PdfObjectId>, &'a Object) -> Result<(), PdfError>,
) -> Result<(), PdfError> {
    for (page_number, page_id) in document.get_pages() {
        let Some(page) = document
            .objects
            .get(&page_id)
            .and_then(|object| object.as_dict().ok())
        else {
            continue;
        };
        let Ok(annotations) = page.get(b"Annots") else {
            continue;
        };
        let Some(annotations) =
            resolve_optional(document, annotations, limits.max_reference_depth)?
                .and_then(|object| object.as_array().ok())
        else {
            continue;
        };
        for (index, annotation) in annotations.iter().enumerate() {
            let object_id = annotation.as_reference().ok();
            if object_id.is_some_and(|id| !inspected.insert(id)) {
                continue;
            }
            let Some(resolved) =
                resolve_optional(document, annotation, limits.max_reference_depth)?
            else {
                continue;
            };
            visit(page_number, index, object_id.map(Into::into), resolved)?;
        }
    }
    Ok(())
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
