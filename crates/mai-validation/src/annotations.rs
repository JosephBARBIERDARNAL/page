use std::collections::BTreeSet;

use lopdf::{Dictionary, Document, Object};

use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;

#[derive(Clone, Debug, Default)]
pub(crate) struct AnnotationSummary {
    pub(crate) invalid_subtypes: Vec<AnnotationFailure>,
    pub(crate) invalid_opacities: Vec<AnnotationFailure>,
    pub(crate) invalid_flags: Vec<AnnotationFailure>,
    pub(crate) color_uses: Vec<AnnotationFailure>,
    pub(crate) invalid_appearance_entries: Vec<AnnotationFailure>,
    pub(crate) invalid_button_appearances: Vec<AnnotationFailure>,
    pub(crate) invalid_other_appearances: Vec<AnnotationFailure>,
}

#[derive(Clone, Debug)]
pub(crate) struct AnnotationFailure {
    pub(crate) object_id: Option<PdfObjectId>,
    pub(crate) description: String,
}

pub(crate) fn inspect(
    document: &Document,
    limits: &SafetyLimits,
) -> Result<AnnotationSummary, PdfError> {
    let mut summary = AnnotationSummary::default();
    let mut inspected = BTreeSet::new();
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
            let Some(dictionary) =
                resolve_optional(document, annotation, limits.max_reference_depth)?
                    .and_then(|object| object.as_dict().ok())
            else {
                continue;
            };
            inspect_annotation(
                document,
                dictionary,
                object_id.map(Into::into),
                &format!("annotation {index} on page {page_number}"),
                limits,
                &mut summary,
            )?;
        }
    }
    Ok(summary)
}

fn inspect_annotation(
    document: &Document,
    annotation: &Dictionary,
    object_id: Option<PdfObjectId>,
    context: &str,
    limits: &SafetyLimits,
    summary: &mut AnnotationSummary,
) -> Result<(), PdfError> {
    let subtype = annotation
        .get(b"Subtype")
        .ok()
        .and_then(|value| value.as_name().ok());
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

    if annotation
        .get(b"CA")
        .ok()
        .and_then(|value| value.as_float().ok())
        .is_some_and(|value| value != 1.0)
    {
        summary.invalid_opacities.push(annotation_failure(
            object_id,
            context,
            "has /CA other than 1",
        ));
    }

    let flags = annotation
        .get(b"F")
        .ok()
        .and_then(|value| value.as_i64().ok());
    if !flags
        .is_some_and(|flags| flags & 4 == 4 && flags & 1 == 0 && flags & 2 == 0 && flags & 32 == 0)
    {
        summary.invalid_flags.push(annotation_failure(
            object_id,
            context,
            "is not printable or is hidden, invisible, or not viewable",
        ));
    }

    if annotation.has(b"C") || annotation.has(b"IC") {
        summary
            .color_uses
            .push(annotation_failure(object_id, context, "contains /C or /IC"));
    }

    let Ok(appearance) = annotation.get(b"AP") else {
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
            .is_none_or(Dictionary::is_empty)
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

fn inherited_field_type<'a>(
    document: &'a Document,
    mut dictionary: &'a Dictionary,
    limits: &SafetyLimits,
) -> Result<Option<&'a [u8]>, PdfError> {
    let mut visited = BTreeSet::new();
    for _ in 0..=limits.max_reference_depth {
        if let Ok(field_type) = dictionary.get(b"FT") {
            return Ok(field_type.as_name().ok());
        }
        let Ok(parent) = dictionary.get(b"Parent") else {
            return Ok(None);
        };
        if let Object::Reference(object_id) = parent
            && !visited.insert(*object_id)
        {
            return Err(PdfError::ReferenceDepth(limits.max_reference_depth));
        }
        let Some(parent) = resolve_optional(document, parent, limits.max_reference_depth)?
            .and_then(|object| object.as_dict().ok())
        else {
            return Ok(None);
        };
        dictionary = parent;
    }
    Err(PdfError::ReferenceDepth(limits.max_reference_depth))
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

fn annotation_failure(
    object_id: Option<PdfObjectId>,
    context: &str,
    detail: &str,
) -> AnnotationFailure {
    AnnotationFailure {
        object_id,
        description: format!("{context} {detail}"),
    }
}
