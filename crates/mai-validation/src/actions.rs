use std::collections::BTreeSet;

use lopdf::{Document, Object, ObjectId};

use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::object_resolution::resolve_optional;

const CATALOG_ACTION_KEYS: &[&[u8]] = &[b"WC", b"WS", b"DS", b"WP", b"DP"];
const PAGE_ACTION_KEYS: &[&[u8]] = &[b"O", b"C"];
const ANNOTATION_ACTION_KEYS: &[&[u8]] = &[
    b"E", b"X", b"D", b"U", b"Fo", b"Bl", b"PO", b"PC", b"PV", b"PI",
];
const FIELD_ACTION_KEYS: &[&[u8]] = &[b"K", b"F", b"V", b"C"];

#[derive(Clone, Debug, Default)]
pub(crate) struct ActionSummary {
    pub(crate) invalid_action_types: Vec<ActionFailure>,
    pub(crate) invalid_named_actions: Vec<ActionFailure>,
    pub(crate) widgets_with_actions: Vec<ActionFailure>,
    pub(crate) widgets_with_additional_actions: Vec<ActionFailure>,
    pub(crate) fields_with_additional_actions: Vec<ActionFailure>,
    pub(crate) catalog_with_additional_actions: Vec<ActionFailure>,
}

#[derive(Clone, Debug)]
pub(crate) struct ActionFailure {
    pub(crate) object_id: Option<PdfObjectId>,
    pub(crate) description: String,
}

pub(crate) fn inspect(
    document: &Document,
    limits: &SafetyLimits,
) -> Result<ActionSummary, PdfError> {
    let Some(catalog_value) = document.trailer.get(b"Root").ok() else {
        return Ok(ActionSummary::default());
    };
    let catalog_id = catalog_value.as_reference().ok().map(Into::into);
    let Some(catalog) = resolve_optional(document, catalog_value, limits.max_reference_depth)?
        .and_then(|object| object.as_dict().ok())
    else {
        return Ok(ActionSummary::default());
    };

    let mut inspector = Inspector {
        document,
        limits,
        summary: ActionSummary::default(),
        seen_actions: BTreeSet::new(),
        seen_annotations: BTreeSet::new(),
        seen_fields: BTreeSet::new(),
        seen_outlines: BTreeSet::new(),
    };
    if catalog.has(b"AA") {
        inspector
            .summary
            .catalog_with_additional_actions
            .push(ActionFailure {
                object_id: catalog_id,
                description: "document catalog contains /AA".to_owned(),
            });
    }
    if let Ok(action) = catalog.get(b"OpenAction") {
        inspector.inspect_action_value(action, "catalog /OpenAction", 0)?;
    }
    inspector.inspect_additional_actions(
        catalog.get(b"AA").ok(),
        CATALOG_ACTION_KEYS,
        "catalog /AA",
        0,
    )?;
    inspector.inspect_outlines(catalog.get(b"Outlines").ok())?;
    inspector.inspect_pages()?;
    inspector.inspect_acro_form(catalog.get(b"AcroForm").ok())?;
    Ok(inspector.summary)
}

struct Inspector<'a> {
    document: &'a Document,
    limits: &'a SafetyLimits,
    summary: ActionSummary,
    seen_actions: BTreeSet<ObjectId>,
    seen_annotations: BTreeSet<ObjectId>,
    seen_fields: BTreeSet<ObjectId>,
    seen_outlines: BTreeSet<ObjectId>,
}

impl Inspector<'_> {
    fn inspect_pages(&mut self) -> Result<(), PdfError> {
        for (page_number, page_id) in self.document.get_pages() {
            let Some(page) = self
                .document
                .objects
                .get(&page_id)
                .and_then(|object| object.as_dict().ok())
            else {
                continue;
            };
            self.inspect_additional_actions(
                page.get(b"AA").ok(),
                PAGE_ACTION_KEYS,
                &format!("page {page_number} /AA"),
                0,
            )?;
            let Some(annotations) = page
                .get(b"Annots")
                .ok()
                .map(|value| {
                    resolve_optional(self.document, value, self.limits.max_reference_depth)
                })
                .transpose()?
                .flatten()
                .and_then(|object| object.as_array().ok())
            else {
                continue;
            };
            for (index, annotation) in annotations.iter().enumerate() {
                self.inspect_annotation(
                    annotation,
                    &format!("annotation {index} on page {page_number}"),
                )?;
            }
        }
        Ok(())
    }

    fn inspect_annotation(&mut self, value: &Object, context: &str) -> Result<(), PdfError> {
        let object_id = value.as_reference().ok();
        if object_id.is_some_and(|id| !self.seen_annotations.insert(id)) {
            return Ok(());
        }
        let Some(annotation) =
            resolve_optional(self.document, value, self.limits.max_reference_depth)?
                .and_then(|object| object.as_dict().ok())
        else {
            return Ok(());
        };
        let failure_id = object_id.map(Into::into);
        let is_widget = annotation
            .get(b"Subtype")
            .ok()
            .and_then(|value| value.as_name().ok())
            == Some(b"Widget".as_slice());
        if is_widget && annotation.has(b"A") {
            self.summary.widgets_with_actions.push(ActionFailure {
                object_id: failure_id,
                description: format!("{context} is a Widget containing /A"),
            });
        }
        if is_widget && annotation.has(b"AA") {
            self.summary
                .widgets_with_additional_actions
                .push(ActionFailure {
                    object_id: failure_id,
                    description: format!("{context} is a Widget containing /AA"),
                });
        }
        if let Ok(action) = annotation.get(b"A") {
            self.inspect_action_value(action, &format!("{context} /A"), 0)?;
        }
        self.inspect_additional_actions(
            annotation.get(b"AA").ok(),
            ANNOTATION_ACTION_KEYS,
            &format!("{context} /AA"),
            0,
        )
    }

    fn inspect_acro_form(&mut self, value: Option<&Object>) -> Result<(), PdfError> {
        let Some(acro_form) = value
            .map(|value| resolve_optional(self.document, value, self.limits.max_reference_depth))
            .transpose()?
            .flatten()
            .and_then(|object| object.as_dict().ok())
        else {
            return Ok(());
        };
        let Some(fields) = acro_form
            .get(b"Fields")
            .ok()
            .map(|value| resolve_optional(self.document, value, self.limits.max_reference_depth))
            .transpose()?
            .flatten()
            .and_then(|object| object.as_array().ok())
        else {
            return Ok(());
        };
        for (index, field) in fields.iter().enumerate() {
            self.inspect_field(field, &format!("AcroForm field {index}"), 0, true)?;
        }
        Ok(())
    }

    fn inspect_field(
        &mut self,
        value: &Object,
        context: &str,
        depth: usize,
        top_level: bool,
    ) -> Result<(), PdfError> {
        self.ensure_depth(depth)?;
        let object_id = value.as_reference().ok();
        if object_id.is_some_and(|id| !self.seen_fields.insert(id)) {
            return Ok(());
        }
        let Some(field) = resolve_optional(self.document, value, self.limits.max_reference_depth)?
            .and_then(|object| object.as_dict().ok())
        else {
            return Ok(());
        };
        // veraPDF accepts every dictionary in AcroForm /Fields as a top-level
        // form field, while child /Kids entries instantiate a field only when
        // the dictionary contains /T.
        if !top_level && !field.has(b"T") {
            return Ok(());
        }
        if field.has(b"AA") {
            self.summary
                .fields_with_additional_actions
                .push(ActionFailure {
                    object_id: object_id.map(Into::into),
                    description: format!("{context} contains /AA"),
                });
        }
        self.inspect_additional_actions(
            field.get(b"AA").ok(),
            FIELD_ACTION_KEYS,
            &format!("{context} /AA"),
            depth,
        )?;
        let Some(kids) = field
            .get(b"Kids")
            .ok()
            .map(|value| resolve_optional(self.document, value, self.limits.max_reference_depth))
            .transpose()?
            .flatten()
            .and_then(|object| object.as_array().ok())
        else {
            return Ok(());
        };
        for (index, kid) in kids.iter().enumerate() {
            self.inspect_field(kid, &format!("{context} child {index}"), depth + 1, false)?;
        }
        Ok(())
    }

    fn inspect_outlines(&mut self, value: Option<&Object>) -> Result<(), PdfError> {
        let Some(outlines) = value
            .map(|value| resolve_optional(self.document, value, self.limits.max_reference_depth))
            .transpose()?
            .flatten()
            .and_then(|object| object.as_dict().ok())
        else {
            return Ok(());
        };
        if let Ok(first) = outlines.get(b"First") {
            self.inspect_outline(first, "outline item", 0)?;
        }
        Ok(())
    }

    fn inspect_outline(
        &mut self,
        value: &Object,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        self.ensure_depth(depth)?;
        let object_id = value.as_reference().ok();
        if object_id.is_some_and(|id| !self.seen_outlines.insert(id)) {
            return Ok(());
        }
        let Some(outline) =
            resolve_optional(self.document, value, self.limits.max_reference_depth)?
                .and_then(|object| object.as_dict().ok())
        else {
            return Ok(());
        };
        if let Ok(action) = outline.get(b"A") {
            self.inspect_action_value(action, &format!("{context} /A"), depth)?;
        }
        if let Ok(first) = outline.get(b"First") {
            self.inspect_outline(first, &format!("{context} child"), depth + 1)?;
        }
        if let Ok(next) = outline.get(b"Next") {
            self.inspect_outline(next, &format!("{context} sibling"), depth + 1)?;
        }
        Ok(())
    }

    fn inspect_additional_actions(
        &mut self,
        value: Option<&Object>,
        action_keys: &[&[u8]],
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        let Some(actions) = value
            .map(|value| resolve_optional(self.document, value, self.limits.max_reference_depth))
            .transpose()?
            .flatten()
            .and_then(|object| object.as_dict().ok())
        else {
            return Ok(());
        };
        for key in action_keys {
            if let Ok(action) = actions.get(key) {
                self.inspect_action_value(
                    action,
                    &format!("{context} /{}", String::from_utf8_lossy(key)),
                    depth,
                )?;
            }
        }
        Ok(())
    }

    fn inspect_action_value(
        &mut self,
        value: &Object,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        self.ensure_depth(depth)?;
        let object_id = value.as_reference().ok();
        if object_id.is_some_and(|id| !self.seen_actions.insert(id)) {
            return Ok(());
        }
        let Some(action) = resolve_optional(self.document, value, self.limits.max_reference_depth)?
            .and_then(|object| object.as_dict().ok())
        else {
            return Ok(());
        };
        let failure_id = object_id.map(Into::into);
        let subtype = action.get(b"S").ok().and_then(|value| value.as_name().ok());
        if !matches!(
            subtype,
            Some(b"GoTo" | b"GoToR" | b"Thread" | b"URI" | b"Named" | b"SubmitForm")
        ) {
            self.summary.invalid_action_types.push(ActionFailure {
                object_id: failure_id,
                description: format!("{context} has a missing or forbidden /S"),
            });
        }
        if subtype == Some(b"Named".as_slice())
            && !matches!(
                action.get(b"N").ok().and_then(|value| value.as_name().ok()),
                Some(b"NextPage" | b"PrevPage" | b"FirstPage" | b"LastPage")
            )
        {
            self.summary.invalid_named_actions.push(ActionFailure {
                object_id: failure_id,
                description: format!("{context} has a missing or forbidden named action /N"),
            });
        }

        let Ok(next_value) = action.get(b"Next") else {
            return Ok(());
        };
        let Some(next) =
            resolve_optional(self.document, next_value, self.limits.max_reference_depth)?
        else {
            return Ok(());
        };
        if next.as_dict().is_ok() {
            self.inspect_action_value(next_value, &format!("{context} /Next"), depth + 1)?;
        } else if let Ok(next_actions) = next.as_array() {
            for (index, next_action) in next_actions.iter().enumerate() {
                self.inspect_action_value(
                    next_action,
                    &format!("{context} /Next[{index}]"),
                    depth + 1,
                )?;
            }
        }
        Ok(())
    }

    fn ensure_depth(&self, depth: usize) -> Result<(), PdfError> {
        if depth > self.limits.max_reference_depth {
            Err(PdfError::ReferenceDepth(self.limits.max_reference_depth))
        } else {
            Ok(())
        }
    }
}
