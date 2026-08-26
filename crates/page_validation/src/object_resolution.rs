use lopdf::{Dictionary, Document, Object, ObjectId};

use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;

/// Number of reference hops tracked without a heap allocation before cycle
/// detection spills to a `Vec`. Real PDF reference chains almost always
/// resolve within a couple of hops, so this keeps the hot `resolve`/
/// `walk_inherited` paths allocation-free in the common case while staying
/// correct for arbitrarily deep (and cyclic) chains up to `maximum_depth`.
const INLINE_VISITED_CAPACITY: usize = 8;

/// A cycle-detection set mirroring `BTreeSet<ObjectId>::insert`'s return
/// value, backed by an inline array until more than
/// `INLINE_VISITED_CAPACITY` distinct ids are seen.
enum Visited {
    Inline {
        ids: [ObjectId; INLINE_VISITED_CAPACITY],
        len: usize,
    },
    Spilled(Vec<ObjectId>),
}

impl Visited {
    fn new() -> Self {
        Self::Inline {
            ids: [(0, 0); INLINE_VISITED_CAPACITY],
            len: 0,
        }
    }

    /// Returns `false` if `id` was already present, `true` if newly recorded.
    fn insert(&mut self, id: ObjectId) -> bool {
        match self {
            Self::Inline { ids, len } => {
                if ids.iter().take(*len).any(|candidate| candidate == &id) {
                    return false;
                }
                if *len < INLINE_VISITED_CAPACITY {
                    let Some(slot) = ids.get_mut(*len) else {
                        return false;
                    };
                    *slot = id;
                    *len += 1;
                } else {
                    let mut spilled = ids.iter().take(*len).copied().collect::<Vec<_>>();
                    spilled.push(id);
                    *self = Self::Spilled(spilled);
                }
                true
            }
            Self::Spilled(ids) => {
                if ids.contains(&id) {
                    return false;
                }
                ids.push(id);
                true
            }
        }
    }
}

pub(crate) fn resolve<'a>(
    document: &'a Document,
    mut object: &'a Object,
    maximum_depth: usize,
) -> Result<&'a Object, PdfError> {
    let mut visited = Visited::new();
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

pub(crate) fn has_non_empty_string_entry(
    document: &Document,
    dictionary: &Dictionary,
    key: &[u8],
    maximum_depth: usize,
) -> Result<bool, PdfError> {
    let Some(value) = dictionary
        .get(key)
        .ok()
        .map(|value| resolve_optional(document, value, maximum_depth))
        .transpose()?
        .flatten()
    else {
        return Ok(false);
    };
    Ok(value.as_str().is_ok_and(|value| !value.is_empty()))
}

/// Whether `dictionary` has `key` as a *meaningfully present* entry for a
/// veraPDF `containsX` boolean predicate: a direct `Object::Null` value is
/// treated as absent, matching veraPDF's own convention confirmed against
/// 1.30.2 for every `containsX`/`isXPresent` predicate this crate checks
/// (`containsEmbeddedFiles`, `isOptionalContentPresent`, `containsEF`,
/// `containsAA`, `containsA`, `containsTR2`, `containsOPI`, ...).
///
/// `Dictionary::has` alone does not make this distinction — it is pure key
/// presence — so it is the wrong primitive for any `containsX` predicate:
/// use this instead. (Several call sites used bare `.has(` for this purpose
/// and were confirmed, then fixed, to wrongly fail a direct-null value.)
pub(crate) fn contains_key(dictionary: &Dictionary, key: &[u8]) -> bool {
    dictionary
        .get(key)
        .is_ok_and(|value| !matches!(value, Object::Null))
}

pub(crate) fn resolved_name<'a>(
    document: &'a Document,
    dictionary: &'a Dictionary,
    key: &[u8],
    maximum_depth: usize,
) -> Result<Option<&'a [u8]>, PdfError> {
    let Ok(value) = dictionary.get(key) else {
        return Ok(None);
    };
    Ok(resolve_optional(document, value, maximum_depth)?.and_then(|value| value.as_name().ok()))
}

pub(crate) fn resolved_bool(
    document: &Document,
    dictionary: &Dictionary,
    key: &[u8],
    maximum_depth: usize,
) -> Result<Option<bool>, PdfError> {
    let Ok(value) = dictionary.get(key) else {
        return Ok(None);
    };
    Ok(resolve_optional(document, value, maximum_depth)?.and_then(|value| value.as_bool().ok()))
}

pub(crate) fn resolved_integer(
    document: &Document,
    dictionary: &Dictionary,
    key: &[u8],
    maximum_depth: usize,
) -> Result<Option<i64>, PdfError> {
    let Ok(value) = dictionary.get(key) else {
        return Ok(None);
    };
    Ok(resolve_optional(document, value, maximum_depth)?.and_then(|value| value.as_i64().ok()))
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
    let mut visited = Visited::new();
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
        let Some(parent) = dictionary_based(parent) else {
            return Ok(None);
        };
        node = parent;
    }
    Err(PdfError::ReferenceDepth(limits.max_reference_depth))
}
