use std::collections::BTreeSet;

use lopdf::{Dictionary, Document, Object, ObjectId};

use crate::catalog::resolve_catalog;
use crate::content_support::for_each_page_annotation;
use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::object_resolution::{
    dictionary_based, has_non_empty_string_entry, resolve_optional, resolved_integer,
    resolved_name, walk_inherited,
};
use crate::page_tree::PageEntry;
use crate::report::RuleFailure;

#[derive(Clone, Debug, Default)]
pub(crate) struct AnnotationSummary {
    pub(crate) invalid_subtypes: Vec<RuleFailure>,
    pub(crate) invalid_subtypes_pdfa2: Vec<RuleFailure>,
    pub(crate) invalid_opacities: Vec<RuleFailure>,
    pub(crate) invalid_flags: Vec<RuleFailure>,
    pub(crate) invalid_flags_pdfa2: Vec<RuleFailure>,
    pub(crate) missing_flags_pdfa2: Vec<RuleFailure>,
    pub(crate) missing_appearances_pdfa2: Vec<RuleFailure>,
    pub(crate) color_uses: Vec<RuleFailure>,
    pub(crate) invalid_appearance_entries: Vec<RuleFailure>,
    pub(crate) invalid_button_appearances: Vec<RuleFailure>,
    pub(crate) invalid_other_appearances: Vec<RuleFailure>,
    pub(crate) annotations_not_nested_in_annot: Vec<RuleFailure>,
    pub(crate) annotations_missing_contents_or_alt: Vec<RuleFailure>,
    pub(crate) contents_language_failures: Vec<RuleFailure>,
}

pub(crate) fn inspect(
    document: &Document,
    pages: &[PageEntry],
    catalog_contains_lang: bool,
    limits: &SafetyLimits,
) -> Result<AnnotationSummary, PdfError> {
    let mut summary = AnnotationSummary::default();
    let mut inspected = BTreeSet::new();
    for_each_page_annotation(
        document,
        pages,
        limits,
        &mut inspected,
        |page_number, index, object_id, page, annotation| {
            let Some(dictionary) = annotation.as_dict().ok() else {
                return Ok(());
            };
            inspect_annotation(
                document,
                page,
                dictionary,
                object_id,
                &format!("annotation {index} on page {page_number}"),
                catalog_contains_lang,
                limits,
                &mut summary,
            )
        },
    )?;
    Ok(summary)
}

fn inspect_annotation(
    document: &Document,
    page: &Dictionary,
    annotation: &Dictionary,
    object_id: Option<PdfObjectId>,
    context: &str,
    catalog_contains_lang: bool,
    limits: &SafetyLimits,
    summary: &mut AnnotationSummary,
) -> Result<(), PdfError> {
    let subtype = resolved_name(document, annotation, b"Subtype", limits.max_reference_depth)?;
    if !matches!(
        subtype,
        Some(
            b"Text"
                | b"Link"
                | b"FreeText"
                | b"Line"
                | b"Square"
                | b"Circle"
                | b"Highlight"
                | b"Underline"
                | b"Squiggly"
                | b"StrikeOut"
                | b"Stamp"
                | b"Ink"
                | b"Popup"
                | b"Widget"
                | b"PrinterMark"
                | b"TrapNet"
        )
    ) {
        summary.invalid_subtypes.push(annotation_failure(
            object_id,
            context,
            "has a missing or forbidden /Subtype",
        ));
    }

    let annotation_is_exempt = matches!(subtype, Some(b"Widget" | b"PrinterMark" | b"Link"));
    // The official veraPDF predicate also treats hidden and crop-box-outside
    // annotations as artifacts for this rule.
    let hidden = resolved_integer(document, annotation, b"F", limits.max_reference_depth)?
        .is_some_and(|flags| flags & 2 == 2);
    let outside_crop_box = annotation_is_outside_crop_box(document, page, annotation, limits)?;
    if !annotation_is_exempt && !hidden && !outside_crop_box {
        let structure_element = annotation_structure_element(document, annotation, limits)?;
        if annotation_struct_parent_standard_type(document, structure_element, limits)?
            != Some(b"Annot".as_slice())
        {
            summary
                .annotations_not_nested_in_annot
                .push(annotation_failure(
                    object_id,
                    context,
                    "is not nested within an Annot structure element",
                ));
        }
        let has_alt = structure_element
            .map(|structure_element| {
                has_non_empty_string_entry(
                    document,
                    structure_element,
                    b"Alt",
                    limits.max_reference_depth,
                )
            })
            .transpose()?
            .unwrap_or(false);
        if !has_non_empty_string_entry(
            document,
            annotation,
            b"Contents",
            limits.max_reference_depth,
        )? && !has_alt
        {
            summary
                .annotations_missing_contents_or_alt
                .push(annotation_failure(
                    object_id,
                    context,
                    "has neither a non-empty /Contents entry nor a non-empty /Alt entry in its enclosing structure element",
                ));
        }
    }
    if !matches!(
        subtype,
        Some(
            b"Text"
                | b"Link"
                | b"FreeText"
                | b"Line"
                | b"Square"
                | b"Circle"
                | b"Polygon"
                | b"PolyLine"
                | b"Highlight"
                | b"Underline"
                | b"Squiggly"
                | b"StrikeOut"
                | b"Stamp"
                | b"Caret"
                | b"Ink"
                | b"Popup"
                | b"FileAttachment"
                | b"Widget"
                | b"PrinterMark"
                | b"TrapNet"
                | b"Watermark"
                | b"Redact"
        )
    ) {
        summary.invalid_subtypes_pdfa2.push(annotation_failure(
            object_id,
            context,
            "has a missing or forbidden /Subtype",
        ));
    }

    if has_non_null_entry(document, annotation, b"Contents", limits)?
        && !annotation_has_language(document, annotation, limits)?
        && !catalog_contains_lang
    {
        summary.contents_language_failures.push(annotation_failure(
            object_id,
            context,
            "has /Contents without a tagged-structure or catalog /Lang",
        ));
    }

    if annotation
        .get(b"CA")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|value| value.as_float().ok())
        .is_some_and(|value| value != 1.0)
    {
        summary.invalid_opacities.push(annotation_failure(
            object_id,
            context,
            "has /CA other than 1",
        ));
    }

    let flags = resolved_integer(document, annotation, b"F", limits.max_reference_depth)?;
    if subtype != Some(b"Popup".as_slice()) && flags.is_none() {
        summary.missing_flags_pdfa2.push(annotation_failure(
            object_id,
            context,
            "has no required /F annotation flags",
        ));
    }
    let flags_are_valid = flags
        .is_some_and(|flags| flags & 4 == 4 && flags & 1 == 0 && flags & 2 == 0 && flags & 32 == 0);
    if !flags_are_valid {
        summary.invalid_flags.push(annotation_failure(
            object_id,
            context,
            "is not printable or is hidden, invisible, or not viewable",
        ));
    }
    if flags.is_some() && !flags_are_valid {
        summary.invalid_flags_pdfa2.push(annotation_failure(
            object_id,
            context,
            "has invalid PDF/A-2/3 annotation flags",
        ));
    }

    if contains_array(document, annotation, b"C", limits)?
        || contains_array(document, annotation, b"IC", limits)?
    {
        summary
            .color_uses
            .push(annotation_failure(object_id, context, "contains /C or /IC"));
    }

    let Ok(appearance) = annotation.get(b"AP") else {
        if subtype != Some(b"Popup".as_slice())
            && subtype != Some(b"Link".as_slice())
            && !zero_annotation_rect(document, annotation, limits)?
        {
            summary.missing_appearances_pdfa2.push(annotation_failure(
                object_id,
                context,
                "has no required /AP appearance dictionary",
            ));
        }
        return Ok(());
    };
    let Some(appearance) = resolve_optional(document, appearance, limits.max_reference_depth)?
        .and_then(|object| object.as_dict().ok())
    else {
        return Ok(());
    };
    let only_normal =
        appearance.len() == 1 && appearance.iter().next().is_some_and(|(key, _)| key == b"N");
    if !only_normal {
        summary.invalid_appearance_entries.push(annotation_failure(
            object_id,
            context,
            "has an /AP dictionary with entries other than only /N",
        ));
        return Ok(());
    }

    let normal = appearance
        .get(b"N")
        .expect("single normal appearance entry");
    let normal = resolve_optional(document, normal, limits.max_reference_depth)?;
    let field_type = inherited_field_type(document, annotation, limits)?;
    if subtype == Some(b"Widget".as_slice()) && field_type == Some(b"Btn".as_slice()) {
        if normal
            .and_then(|object| object.as_dict().ok())
            .map(|states| contains_appearance_stream(document, states, limits))
            .transpose()?
            .is_none_or(|contains_stream| !contains_stream)
        {
            summary.invalid_button_appearances.push(annotation_failure(
                object_id,
                context,
                "is a button Widget whose normal appearance is not a nonempty subdictionary",
            ));
        }
    } else if normal.is_none_or(|object| object.as_stream().is_err()) {
        summary.invalid_other_appearances.push(annotation_failure(
            object_id,
            context,
            "has a normal appearance that is not a stream",
        ));
    }
    Ok(())
}

fn annotation_struct_parent_standard_type(
    document: &Document,
    structure_element: Option<&Dictionary>,
    limits: &SafetyLimits,
) -> Result<Option<&'static [u8]>, PdfError> {
    let Some(structure_element) = structure_element else {
        return Ok(None);
    };
    let Some(structure_type) = structure_element
        .get(b"S")
        .ok()
        .and_then(|value| value.as_name().ok())
    else {
        return Ok(None);
    };
    if structure_type == b"Annot" {
        return Ok(Some(b"Annot"));
    }

    // A custom structure type may be role-mapped to the standard Annot type.
    let Some(catalog) = resolve_catalog(document, limits)?.map(|catalog| catalog.dictionary) else {
        return Ok(None);
    };
    let Some(struct_tree_root) = catalog
        .get(b"StructTreeRoot")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(dictionary_based)
    else {
        return Ok(None);
    };
    let Some(role_map) = struct_tree_root
        .get(b"RoleMap")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(dictionary_based)
    else {
        return Ok(None);
    };
    let mut current = structure_type;
    for _ in 0..limits.max_object_count {
        if current == b"Annot" {
            return Ok(Some(b"Annot"));
        }
        let Some(mapped) = role_map
            .get(current)
            .ok()
            .map(|value| resolve_optional(document, value, limits.max_reference_depth))
            .transpose()?
            .flatten()
            .and_then(|value| value.as_name().ok())
        else {
            return Ok(None);
        };
        current = mapped;
    }
    Ok(None)
}

fn annotation_structure_element<'a>(
    document: &'a Document,
    annotation: &Dictionary,
    limits: &SafetyLimits,
) -> Result<Option<&'a Dictionary>, PdfError> {
    let Some(struct_parent) = resolved_integer(
        document,
        annotation,
        b"StructParent",
        limits.max_reference_depth,
    )?
    else {
        return Ok(None);
    };
    let Some(catalog) = resolve_catalog(document, limits)?.map(|catalog| catalog.dictionary) else {
        return Ok(None);
    };
    let Some(struct_tree_root) = catalog
        .get(b"StructTreeRoot")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(dictionary_based)
    else {
        return Ok(None);
    };
    let Some(parent_tree) = struct_tree_root
        .get(b"ParentTree")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(dictionary_based)
    else {
        return Ok(None);
    };
    let mut ancestors = BTreeSet::new();
    let mut steps = 0;
    let Some(structure_element) = find_number_tree_entry(
        document,
        parent_tree,
        struct_parent,
        limits,
        &mut ancestors,
        &mut steps,
        0,
    )?
    else {
        return Ok(None);
    };
    Ok(
        resolve_optional(document, structure_element, limits.max_reference_depth)?
            .and_then(|object| object.as_dict().ok()),
    )
}

fn annotation_is_outside_crop_box(
    document: &Document,
    page: &Dictionary,
    annotation: &Dictionary,
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    let Some(rect) = resolved_rectangle(document, annotation, b"Rect", limits)? else {
        return Ok(false);
    };
    let Some(crop_box) = walk_inherited(
        document,
        page,
        limits,
        b"CropBox",
        |document, value, limits| resolve_optional(document, value, limits.max_reference_depth),
    )?
    .or(walk_inherited(
        document,
        page,
        limits,
        b"MediaBox",
        |document, value, limits| resolve_optional(document, value, limits.max_reference_depth),
    )?)
    .and_then(rectangle) else {
        return Ok(false);
    };
    let [left, bottom, right, top] = normalized_rectangle(rect);
    let [crop_left, crop_bottom, crop_right, crop_top] = normalized_rectangle(crop_box);
    Ok(left < crop_left || bottom < crop_bottom || right > crop_right || top > crop_top)
}

fn normalized_rectangle([first_x, first_y, second_x, second_y]: [f64; 4]) -> [f64; 4] {
    [
        first_x.min(second_x),
        first_y.min(second_y),
        first_x.max(second_x),
        first_y.max(second_y),
    ]
}

fn resolved_rectangle(
    document: &Document,
    dictionary: &Dictionary,
    key: &[u8],
    limits: &SafetyLimits,
) -> Result<Option<[f64; 4]>, PdfError> {
    let Some(value) = dictionary
        .get(key)
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
    else {
        return Ok(None);
    };
    Ok(rectangle(value))
}

fn rectangle(value: &Object) -> Option<[f64; 4]> {
    let values = value.as_array().ok()?;
    let [left, bottom, right, top] = values.as_slice() else {
        return None;
    };
    Some([
        object_number(left)?,
        object_number(bottom)?,
        object_number(right)?,
        object_number(top)?,
    ])
}

fn object_number(value: &Object) -> Option<f64> {
    value
        .as_i64()
        .map(|value| value as f64)
        .or_else(|_| value.as_float().map(f64::from))
        .ok()
}

fn zero_annotation_rect(
    document: &Document,
    annotation: &Dictionary,
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    let Some(rect) = annotation
        .get(b"Rect")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|value| value.as_array().ok())
    else {
        return Ok(false);
    };
    if rect.len() != 4 {
        return Ok(false);
    }
    let Some(values) = rect
        .iter()
        .map(|value| value.as_float().ok())
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(false);
    };
    Ok(values[0] == values[2] && values[1] == values[3])
}

fn inherited_field_type<'a>(
    document: &'a Document,
    dictionary: &'a Dictionary,
    limits: &SafetyLimits,
) -> Result<Option<&'a [u8]>, PdfError> {
    walk_inherited(document, dictionary, limits, b"FT", |_, value, _| {
        Ok(
            resolve_optional(document, value, limits.max_reference_depth)?
                .and_then(|value| value.as_name().ok()),
        )
    })
}

fn contains_array(
    document: &Document,
    dictionary: &Dictionary,
    key: &[u8],
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    let Ok(value) = dictionary.get(key) else {
        return Ok(false);
    };
    Ok(
        resolve_optional(document, value, limits.max_reference_depth)?
            .is_some_and(|value| value.as_array().is_ok()),
    )
}

fn has_non_null_entry(
    document: &Document,
    dictionary: &Dictionary,
    key: &[u8],
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    let Ok(value) = dictionary.get(key) else {
        return Ok(false);
    };
    Ok(
        resolve_optional(document, value, limits.max_reference_depth)?
            .is_some_and(|value| !matches!(value, Object::Null)),
    )
}

fn annotation_has_language(
    document: &Document,
    annotation: &Dictionary,
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    // veraPDF's PDAnnot.containsLang comes from the annotation's tagged
    // structure association, not from an arbitrary /Lang key on the annotation.
    let Some(structure_element) = annotation_structure_element(document, annotation, limits)?
    else {
        return Ok(false);
    };
    has_non_null_entry(document, structure_element, b"Lang", limits)
}

fn find_number_tree_entry<'a>(
    document: &'a Document,
    node: &'a Dictionary,
    key: i64,
    limits: &SafetyLimits,
    ancestors: &mut BTreeSet<ObjectId>,
    steps: &mut usize,
    depth: usize,
) -> Result<Option<&'a Object>, PdfError> {
    if depth > limits.max_reference_depth {
        return Err(PdfError::ReferenceDepth(limits.max_reference_depth));
    }
    *steps += 1;
    if *steps > limits.max_object_count {
        return Err(PdfError::TooManyObjects {
            actual: *steps,
            limit: limits.max_object_count,
        });
    }
    if let Ok(nums) = node.get(b"Nums")
        && let Some(nums) = resolve_optional(document, nums, limits.max_reference_depth)?
            .and_then(|value| value.as_array().ok())
    {
        for pair in nums.chunks(2) {
            if pair.len() == 2 && pair[0].as_i64().ok() == Some(key) {
                return Ok(Some(&pair[1]));
            }
        }
    }
    let Some(kids) = node
        .get(b"Kids")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|value| value.as_array().ok())
    else {
        return Ok(None);
    };
    for kid in kids {
        let Some(kid_object) = resolve_optional(document, kid, limits.max_reference_depth)? else {
            continue;
        };
        let Some(kid_dictionary) = kid_object.as_dict().ok() else {
            continue;
        };
        let Some(object_id) = kid.as_reference().ok() else {
            continue;
        };
        if !ancestors.insert(object_id) {
            return Err(PdfError::ReferenceDepth(limits.max_reference_depth));
        }
        let result = find_number_tree_entry(
            document,
            kid_dictionary,
            key,
            limits,
            ancestors,
            steps,
            depth + 1,
        );
        ancestors.remove(&object_id);
        if let Some(value) = result? {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

fn contains_appearance_stream(
    document: &Document,
    states: &Dictionary,
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    for (_, value) in states.iter() {
        if resolve_optional(document, value, limits.max_reference_depth)?
            .is_some_and(|value| value.as_stream().is_ok())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn annotation_failure(object_id: Option<PdfObjectId>, context: &str, detail: &str) -> RuleFailure {
    RuleFailure {
        object_id,
        description: format!("{context} {detail}"),
    }
}
