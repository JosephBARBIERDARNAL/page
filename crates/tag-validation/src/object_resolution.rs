use std::collections::BTreeSet;

use lopdf::{Dictionary, Document, Object, ObjectId};

use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;

pub(crate) fn resolve<'a>(
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

/// Resolves an object while treating a missing or malformed indirect target as
/// absent. Reference cycles and overlong chains remain operational failures.
pub(crate) fn resolve_optional<'a>(
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

pub(crate) fn dictionary_based(object: &Object) -> Option<&Dictionary> {
    match object {
        Object::Dictionary(dictionary) => Some(dictionary),
        Object::Stream(stream) => Some(&stream.dict),
        _ => None,
    }
}

/// Identifies a discovered resource (an ICCBased profile, a used font, ...)
/// either by its indirect object id or, for a direct value with no id of
/// its own, by a caller-chosen description used only for deduplication.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ResourceKey {
    Indirect(ObjectId),
    Direct(String),
}

impl ResourceKey {
    pub(crate) fn object_id(&self) -> Option<PdfObjectId> {
        match self {
            Self::Indirect(id) => Some((*id).into()),
            Self::Direct(_) => None,
        }
    }
}

/// Walks a dictionary's `/Parent` chain looking for `key`, bounded by
/// `limits.max_reference_depth` and guarded against reference cycles.
/// `extract` interprets the found value once `key` is present, without
/// re-walking `/Parent` itself.
pub(crate) fn walk_inherited<'a, T>(
    document: &'a Document,
    mut node: &'a Dictionary,
    limits: &SafetyLimits,
    key: &[u8],
    extract: impl Fn(&'a Document, &'a Object, &SafetyLimits) -> Result<Option<T>, PdfError>,
) -> Result<Option<T>, PdfError> {
    let mut visited = BTreeSet::new();
    for _ in 0..=limits.max_reference_depth {
        if let Ok(value) = node.get(key) {
            return extract(document, value, limits);
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
