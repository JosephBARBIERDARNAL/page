use lopdf::{Dictionary, Document, Object};

use crate::model::PdfObjectId;

const MIN_INTEGER: i64 = -2_147_483_648;
const MAX_INTEGER: i64 = 2_147_483_647;
const MAX_REAL: f32 = 32_767.0;
const MAX_STRING_BYTES: usize = 65_535;
const MAX_NAME_BYTES: usize = 127;
const MAX_ARRAY_ENTRIES: usize = 8_191;
const MAX_DICTIONARY_ENTRIES: usize = 4_095;
const MAX_INDIRECT_OBJECTS: usize = 8_388_607;

#[derive(Clone, Debug, Default)]
pub(crate) struct ObjectLimitsSummary {
    pub(crate) out_of_range_integers: Vec<PdfObjectId>,
    pub(crate) out_of_range_reals: Vec<PdfObjectId>,
    pub(crate) overlong_strings: Vec<PdfObjectId>,
    pub(crate) overlong_names: Vec<PdfObjectId>,
    pub(crate) oversized_arrays: Vec<PdfObjectId>,
    pub(crate) oversized_dictionaries: Vec<PdfObjectId>,
    pub(crate) too_many_indirect_objects: bool,
}

pub(crate) fn inspect(document: &Document) -> ObjectLimitsSummary {
    let mut summary = ObjectLimitsSummary {
        too_many_indirect_objects: exceeds_indirect_object_limit(document.objects.len()),
        ..ObjectLimitsSummary::default()
    };

    // Every indirect object is inspected once. References are deliberately not
    // followed here: their targets are themselves entries in `objects`, while
    // recursively following them would report the same direct subtree more
    // than once and would need cycle handling unrelated to these predicates.
    for (object_id, object) in &document.objects {
        inspect_object(object, (*object_id).into(), &mut summary);
    }
    inspect_dictionary(&document.trailer, None, &mut summary);
    summary
}

fn inspect_object(object: &Object, object_id: PdfObjectId, summary: &mut ObjectLimitsSummary) {
    match object {
        Object::Integer(value) if !(*value >= MIN_INTEGER && *value <= MAX_INTEGER) => {
            summary.out_of_range_integers.push(object_id);
        }
        Object::Real(value) if !(*value >= -MAX_REAL && *value <= MAX_REAL) => {
            summary.out_of_range_reals.push(object_id);
        }
        Object::String(value, _) if value.len() > MAX_STRING_BYTES => {
            summary.overlong_strings.push(object_id);
        }
        Object::Name(value) if value.len() > MAX_NAME_BYTES => {
            summary.overlong_names.push(object_id);
        }
        Object::Array(values) => {
            if values.len() > MAX_ARRAY_ENTRIES {
                summary.oversized_arrays.push(object_id);
            }
            for value in values {
                inspect_object(value, object_id, summary);
            }
        }
        Object::Dictionary(dictionary) => inspect_dictionary(dictionary, Some(object_id), summary),
        Object::Stream(stream) => inspect_dictionary(&stream.dict, Some(object_id), summary),
        Object::Null | Object::Boolean(_) | Object::Reference(_) => {}
        Object::Integer(_) | Object::Real(_) | Object::String(_, _) | Object::Name(_) => {}
    }
}

fn inspect_dictionary(
    dictionary: &Dictionary,
    object_id: Option<PdfObjectId>,
    summary: &mut ObjectLimitsSummary,
) {
    // The pinned COS model omits direct null dictionary entries. This is
    // observable in §6.1.12 test 6, whose `size` therefore counts only the
    // non-null entries.
    if dictionary
        .iter()
        .filter(|(_, value)| !matches!(value, Object::Null))
        .count()
        > MAX_DICTIONARY_ENTRIES
        && let Some(object_id) = object_id
    {
        summary.oversized_dictionaries.push(object_id);
    }
    for (_, value) in dictionary.iter() {
        if let Some(object_id) = object_id {
            inspect_object(value, object_id, summary);
        }
    }
}

pub(crate) const fn exceeds_indirect_object_limit(count: usize) -> bool {
    count > MAX_INDIRECT_OBJECTS
}

#[cfg(test)]
mod tests {
    use super::exceeds_indirect_object_limit;

    #[test]
    fn indirect_object_limit_is_inclusive() {
        assert!(!exceeds_indirect_object_limit(8_388_607));
        assert!(exceeds_indirect_object_limit(8_388_608));
    }
}
