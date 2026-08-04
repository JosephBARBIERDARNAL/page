use std::collections::{BTreeMap, BTreeSet};

use lopdf::{Document, Object, ObjectId};

use crate::catalog::resolve_catalog;
use crate::error::PdfError;
use crate::file_spec;
use crate::limits::SafetyLimits;
use crate::object_resolution::{contains_key, dictionary_based, resolve_optional, resolved_name};
use crate::page_tree::PageEntry;
use crate::report::RuleFailure;

const CATALOG_ACTION_KEYS: &[&[u8]] = &[b"WC", b"WS", b"DS", b"WP", b"DP"];
const PAGE_ACTION_KEYS: &[&[u8]] = &[b"O", b"C"];
const ANNOTATION_ACTION_KEYS: &[&[u8]] = &[
    b"E", b"X", b"D", b"U", b"Fo", b"Bl", b"PO", b"PC", b"PV", b"PI",
];
const FIELD_ACTION_KEYS: &[&[u8]] = &[b"K", b"F", b"V", b"C"];

#[derive(Clone, Debug, Default)]
pub(crate) struct ActionSummary {
    pub(crate) invalid_action_types: Vec<RuleFailure>,
    pub(crate) invalid_named_actions: Vec<RuleFailure>,
    pub(crate) widgets_with_actions: Vec<RuleFailure>,
    pub(crate) widgets_with_additional_actions: Vec<RuleFailure>,
    pub(crate) fields_with_additional_actions: Vec<RuleFailure>,
    pub(crate) catalog_with_additional_actions: Vec<RuleFailure>,
    pub(crate) file_specs_with_embedded_files: Vec<RuleFailure>,
}

pub(crate) fn inspect(
    document: &Document,
    pages: &BTreeMap<u32, PageEntry>,
    limits: &SafetyLimits,
) -> Result<ActionSummary, PdfError> {
    let Some(catalog) = resolve_catalog(document, limits)? else {
        return Ok(ActionSummary::default());
    };
    let catalog_id = Some(catalog.object_id);
    let catalog = catalog.dictionary;

    let mut inspector = Inspector {
        document,
        pages,
        limits,
        summary: ActionSummary::default(),
        seen_actions: BTreeSet::new(),
        seen_annotations: BTreeSet::new(),
        seen_fields: BTreeSet::new(),
        seen_outlines: BTreeSet::new(),
    };
    if contains_key(catalog, b"AA") {
        inspector
            .summary
            .catalog_with_additional_actions
            .push(RuleFailure {
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
        false,
    )?;
    inspector.inspect_outlines(catalog.get(b"Outlines").ok())?;
    inspector.inspect_pages()?;
    inspector.inspect_acro_form(catalog.get(b"AcroForm").ok())?;
    Ok(inspector.summary)
}

struct Inspector<'a> {
    document: &'a Document,
    pages: &'a BTreeMap<u32, PageEntry>,
    limits: &'a SafetyLimits,
    summary: ActionSummary,
    seen_actions: BTreeSet<ObjectId>,
    seen_annotations: BTreeSet<ObjectId>,
    seen_fields: BTreeSet<ObjectId>,
    seen_outlines: BTreeSet<ObjectId>,
}

impl Inspector<'_> {
    // Not routed through `content_support::for_each_page_annotation`: that
    // helper's visit callback would need `&mut self` for the recursive
    // `inspect_action_value`/`inspect_additional_actions` calls below, which
    // conflicts with also passing `&mut self.seen_annotations` as a separate
    // argument.
    fn inspect_pages(&mut self) -> Result<(), PdfError> {
        let pages = self.pages;
        for (&page_number, page_entry) in pages {
            let Some(page) = page_entry.resolve(self.document) else {
                continue;
            };
            self.inspect_additional_actions(
                page.get(b"AA").ok(),
                PAGE_ACTION_KEYS,
                &format!("page {page_number} /AA"),
                0,
                false,
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
        let Some(annotation) =
            resolve_optional(self.document, value, self.limits.max_reference_depth)?
                .and_then(|object| object.as_dict().ok())
        else {
            return Ok(());
        };
        if object_id.is_some_and(|id| !self.seen_annotations.insert(id)) {
            return Ok(());
        }
        let failure_id = object_id.map(Into::into);
        let is_widget = resolved_name(
            self.document,
            annotation,
            b"Subtype",
            self.limits.max_reference_depth,
        )? == Some(b"Widget".as_slice());
        if is_widget && contains_key(annotation, b"A") {
            self.summary.widgets_with_actions.push(RuleFailure {
                object_id: failure_id,
                description: format!("{context} is a Widget containing /A"),
            });
        }
        if is_widget && contains_key(annotation, b"AA") {
            self.summary
                .widgets_with_additional_actions
                .push(RuleFailure {
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
            false,
        )
    }

    fn inspect_acro_form(&mut self, value: Option<&Object>) -> Result<(), PdfError> {
        let Some(acro_form) = value
            .map(|value| resolve_optional(self.document, value, self.limits.max_reference_depth))
            .transpose()?
            .flatten()
            .and_then(dictionary_based)
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
        let Some(field) = resolve_optional(self.document, value, self.limits.max_reference_depth)?
            .and_then(dictionary_based)
        else {
            return Ok(());
        };
        // veraPDF accepts every dictionary in AcroForm /Fields as a top-level
        // form field, while child /Kids entries instantiate a field only when
        // the dictionary contains /T.
        if !top_level && !contains_key(field, b"T") {
            return Ok(());
        }
        if object_id.is_some_and(|id| !self.seen_fields.insert(id)) {
            return Ok(());
        }
        if contains_key(field, b"AA") {
            self.summary
                .fields_with_additional_actions
                .push(RuleFailure {
                    object_id: object_id.map(Into::into),
                    description: format!("{context} contains /AA"),
                });
        }
        self.inspect_additional_actions(
            field.get(b"AA").ok(),
            FIELD_ACTION_KEYS,
            &format!("{context} /AA"),
            depth,
            true,
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
            .and_then(dictionary_based)
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
        let Some(outline) =
            resolve_optional(self.document, value, self.limits.max_reference_depth)?
                .and_then(dictionary_based)
        else {
            return Ok(());
        };
        if object_id.is_some_and(|id| !self.seen_outlines.insert(id)) {
            return Ok(());
        }
        if let Ok(action) = outline.get(b"A") {
            self.inspect_action_value_with_shape(action, &format!("{context} /A"), depth, true)?;
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
        dictionary_based_source: bool,
    ) -> Result<(), PdfError> {
        let Some(actions) = value
            .map(|value| resolve_optional(self.document, value, self.limits.max_reference_depth))
            .transpose()?
            .flatten()
            .and_then(|object| {
                if dictionary_based_source {
                    dictionary_based(object)
                } else {
                    object.as_dict().ok()
                }
            })
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
        self.inspect_action_value_with_shape(value, context, depth, false)
    }

    fn inspect_action_value_with_shape(
        &mut self,
        value: &Object,
        context: &str,
        depth: usize,
        dictionary_based_source: bool,
    ) -> Result<(), PdfError> {
        self.ensure_depth(depth)?;
        let object_id = value.as_reference().ok();
        let Some(action) = resolve_optional(self.document, value, self.limits.max_reference_depth)?
            .and_then(|object| {
                if dictionary_based_source {
                    dictionary_based(object)
                } else {
                    object.as_dict().ok()
                }
            })
        else {
            return Ok(());
        };
        if object_id.is_some_and(|id| !self.seen_actions.insert(id)) {
            return Ok(());
        }
        let failure_id = object_id.map(Into::into);
        let subtype = resolved_name(self.document, action, b"S", self.limits.max_reference_depth)?;
        if !matches!(
            subtype,
            Some(b"GoTo" | b"GoToR" | b"Thread" | b"URI" | b"Named" | b"SubmitForm")
        ) {
            self.summary.invalid_action_types.push(RuleFailure {
                object_id: failure_id,
                description: format!("{context} has a missing or forbidden /S"),
            });
        }
        if subtype == Some(b"Named".as_slice())
            && !matches!(
                resolved_name(self.document, action, b"N", self.limits.max_reference_depth,)?,
                Some(b"NextPage" | b"PrevPage" | b"FirstPage" | b"LastPage")
            )
        {
            self.summary.invalid_named_actions.push(RuleFailure {
                object_id: failure_id,
                description: format!("{context} has a missing or forbidden named action /N"),
            });
        }
        // GoToR and SubmitForm are the only allowed action types that carry
        // a file specification (/F); veraPDF creates the same
        // CosFileSpecification object, and applies the same containsEF
        // check, for a file spec reached this way as for one reached
        // through the catalog Names/EmbeddedFiles tree (confirmed against
        // veraPDF 1.30.2).
        if matches!(subtype, Some(b"GoToR" | b"SubmitForm"))
            && let Ok(file_spec_value) = action.get(b"F")
            && let Some(failure) = file_spec::inspect(
                self.document,
                file_spec_value,
                self.limits,
                &format!("{context} /F"),
            )?
        {
            self.summary.file_specs_with_embedded_files.push(failure);
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
