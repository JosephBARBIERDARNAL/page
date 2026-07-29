use std::collections::BTreeSet;

use lopdf::{Document, Object};

use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;

#[derive(Clone, Debug, Default)]
pub(crate) struct StreamSafetySummary {
    pub(crate) external_stream_entries: Vec<StreamFailure>,
    pub(crate) lzw_filters: Vec<PdfObjectId>,
}

#[derive(Clone, Debug)]
pub(crate) struct StreamFailure {
    pub(crate) object_id: PdfObjectId,
    pub(crate) keys: Vec<&'static str>,
}

pub(crate) fn inspect(
    document: &Document,
    limits: &SafetyLimits,
) -> Result<StreamSafetySummary, PdfError> {
    let mut summary = StreamSafetySummary::default();
    for (object_id, object) in &document.objects {
        let Object::Stream(stream) = object else {
            continue;
        };
        let keys = [
            (b"F".as_slice(), "F"),
            (b"FFilter".as_slice(), "FFilter"),
            (b"FDecodeParms".as_slice(), "FDecodeParms"),
        ]
        .into_iter()
        .filter_map(|(key, name)| {
            stream
                .dict
                .get(key)
                .ok()
                .filter(|value| !matches!(value, Object::Null))
                .map(|_| name)
        })
        .collect::<Vec<_>>();
        if !keys.is_empty() {
            summary.external_stream_entries.push(StreamFailure {
                object_id: (*object_id).into(),
                keys,
            });
        }
        if stream
            .dict
            .get(b"Filter")
            .ok()
            .map(|value| filter_contains_lzw_decode(document, value, limits.max_reference_depth))
            .transpose()?
            .unwrap_or(false)
        {
            summary.lzw_filters.push((*object_id).into());
        }
    }
    Ok(summary)
}

fn filter_contains_lzw_decode(
    document: &Document,
    filter: &Object,
    maximum_depth: usize,
) -> Result<bool, PdfError> {
    let Some(filter) = resolve_optional(document, filter, maximum_depth)? else {
        return Ok(false);
    };
    Ok(match filter {
        Object::Name(name) => name.as_slice() == b"LZWDecode",
        Object::Array(filters) => filters.iter().try_fold(false, |found, filter| {
            Ok::<bool, PdfError>(
                found || filter_contains_lzw_decode(document, filter, maximum_depth)?,
            )
        })?,
        _ => false,
    })
}

fn resolve<'a>(
    document: &'a Document,
    mut object: &'a Object,
    maximum_depth: usize,
) -> Result<&'a Object, PdfError> {
    let mut visited = BTreeSet::new();
    for _ in 0..=maximum_depth {
        let Object::Reference(object_id) = object else {
            return Ok(object);
        };
        if !visited.insert(*object_id) {
            return Err(PdfError::ReferenceDepth(maximum_depth));
        }
        object = document
            .objects
            .get(object_id)
            .ok_or(PdfError::UnexpectedObject("missing indirect object"))?;
    }
    Err(PdfError::ReferenceDepth(maximum_depth))
}

fn resolve_optional<'a>(
    document: &'a Document,
    object: &'a Object,
    maximum_depth: usize,
) -> Result<Option<&'a Object>, PdfError> {
    match resolve(document, object, maximum_depth) {
        Ok(object) => Ok(Some(object)),
        Err(error @ PdfError::ReferenceDepth(_)) => Err(error),
        Err(_) => Ok(None),
    }
}
