use std::collections::BTreeSet;

use lopdf::{Document, Object, ObjectId};

use crate::catalog::resolve_catalog;
use crate::error::PdfError;
use crate::file_spec;
use crate::limits::SafetyLimits;
use crate::model::InspectionNeed;
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
    pub(crate) invalid_action_types_pdfa2: Vec<RuleFailure>,
    pub(crate) invalid_named_actions: Vec<RuleFailure>,
    pub(crate) widgets_with_actions: Vec<RuleFailure>,
    pub(crate) widgets_with_additional_actions: Vec<RuleFailure>,
    pub(crate) fields_with_additional_actions: Vec<RuleFailure>,
    pub(crate) catalog_with_additional_actions: Vec<RuleFailure>,
    pub(crate) pages_with_additional_actions: Vec<RuleFailure>,
    pub(crate) outline_entries: Vec<RuleFailure>,
    pub(crate) file_specs_with_embedded_files: Vec<RuleFailure>,
    pub(crate) file_specs_missing_f_or_uf: Vec<RuleFailure>,
    pub(crate) file_specs_missing_or_empty_f_or_uf: Vec<RuleFailure>,
    pub(crate) media_clips_missing_ct: Vec<RuleFailure>,
    pub(crate) media_clips_missing_alt: Vec<RuleFailure>,
}

pub(crate) fn inspect(
    document: &Document,
    pages: &[PageEntry],
    limits: &SafetyLimits,
    need: InspectionNeed,
) -> Result<ActionSummary, PdfError> {
    if !need.should_run() {
        return Ok(ActionSummary::default());
    }
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
        seen_outlines: BTreeSet::new(),
        seen_media_clips: BTreeSet::new(),
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
    pages: &'a [PageEntry],
    limits: &'a SafetyLimits,
    summary: ActionSummary,
    seen_actions: BTreeSet<ObjectId>,
    seen_annotations: BTreeSet<ObjectId>,
    seen_outlines: BTreeSet<ObjectId>,
    seen_media_clips: BTreeSet<ObjectId>,
}

impl Inspector<'_> {
    // Not routed through `content_support::for_each_page_annotation`: that
    // helper's visit callback would need `&mut self` for the recursive
    // `inspect_action_value`/`inspect_additional_actions` calls below, which
    // conflicts with also passing `&mut self.seen_annotations` as a separate
    // argument.
    fn inspect_pages(&mut self) -> Result<(), PdfError> {
        let pages = self.pages;
        for (index, page_entry) in pages.iter().enumerate() {
            let page_number = (index + 1) as u32;
            let Some(page) = page_entry.resolve(self.document) else {
                continue;
            };
            if contains_key(page, b"AA") {
                self.summary
                    .pages_with_additional_actions
                    .push(RuleFailure {
                        object_id: page_entry.object_id().map(Into::into),
                        description: format!("page {page_number} contains /AA"),
                    });
            }
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
        crate::forms::for_each_form_field(
            self.document,
            acro_form,
            self.limits,
            |field, object_id, context, depth| {
                if contains_key(field, b"AA") {
                    self.summary
                        .fields_with_additional_actions
                        .push(RuleFailure {
                            object_id,
                            description: format!("{context} contains /AA"),
                        });
                }
                self.inspect_additional_actions(
                    field.get(b"AA").ok(),
                    FIELD_ACTION_KEYS,
                    &format!("{context} /AA"),
                    depth,
                    true,
                )
            },
        )
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
        self.summary.outline_entries.push(RuleFailure {
            object_id: object_id.map(Into::into),
            description: format!("{context} is an outline entry"),
        });
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
        if subtype == Some(b"Rendition".as_slice()) {
            self.inspect_rendition_media_clip(action, context)?;
        }
        if !matches!(
            subtype,
            Some(b"GoTo" | b"GoToR" | b"Thread" | b"URI" | b"Named" | b"SubmitForm")
        ) {
            self.summary.invalid_action_types.push(RuleFailure {
                object_id: failure_id,
                description: format!("{context} has a missing or forbidden /S"),
            });
        }
        if !matches!(
            subtype,
            Some(b"GoTo" | b"GoToR" | b"GoToE" | b"Thread" | b"URI" | b"Named" | b"SubmitForm")
        ) {
            self.summary.invalid_action_types_pdfa2.push(RuleFailure {
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
            && let Some(file_spec_dictionary) = resolve_optional(
                self.document,
                file_spec_value,
                self.limits.max_reference_depth,
            )?
            .and_then(dictionary_based)
            && contains_key(file_spec_dictionary, b"EF")
            && (!contains_key(file_spec_dictionary, b"F")
                || !contains_key(file_spec_dictionary, b"UF"))
        {
            self.summary.file_specs_missing_f_or_uf.push(RuleFailure {
                object_id: object_id.map(Into::into),
                description: format!(
                    "{context} /F embedded-file specification is missing /F or /UF"
                ),
            });
        }
        if matches!(subtype, Some(b"GoToR" | b"SubmitForm"))
            && let Ok(file_spec_value) = action.get(b"F")
            && file_spec_value.as_reference().is_err()
            && let Some(file_spec_dictionary) = resolve_optional(
                self.document,
                file_spec_value,
                self.limits.max_reference_depth,
            )?
            .and_then(dictionary_based)
            && contains_key(file_spec_dictionary, b"EF")
            && (!file_spec::has_non_empty_string_entry(
                self.document,
                file_spec_dictionary,
                b"F",
                self.limits.max_reference_depth,
            )? || !file_spec::has_non_empty_string_entry(
                self.document,
                file_spec_dictionary,
                b"UF",
                self.limits.max_reference_depth,
            )?)
        {
            self.summary.file_specs_missing_or_empty_f_or_uf.push(RuleFailure {
                object_id: object_id.map(Into::into),
                description: format!(
                    "{context} /F embedded-file specification is missing or has an empty /F or /UF"
                ),
            });
        }
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

    fn inspect_rendition_media_clip(
        &mut self,
        action: &lopdf::Dictionary,
        context: &str,
    ) -> Result<(), PdfError> {
        // veraPDF exposes PDMediaClip through an MR rendition's /C link.
        // Keep the inspection on the same bounded, reachable action graph
        // instead of scanning arbitrary unreferenced dictionaries.
        let Some(rendition) = action
            .get(b"R")
            .ok()
            .map(|value| resolve_optional(self.document, value, self.limits.max_reference_depth))
            .transpose()?
            .flatten()
            .and_then(dictionary_based)
        else {
            return Ok(());
        };
        if resolved_name(
            self.document,
            rendition,
            b"S",
            self.limits.max_reference_depth,
        )? != Some(b"MR".as_slice())
        {
            return Ok(());
        }
        let Some(media_clip_value) = rendition.get(b"C").ok() else {
            return Ok(());
        };
        let media_clip_object_id = media_clip_value.as_reference().ok();
        let Some(media_clip) = resolve_optional(
            self.document,
            media_clip_value,
            self.limits.max_reference_depth,
        )?
        .and_then(dictionary_based) else {
            return Ok(());
        };
        let has_ct = media_clip
            .get(b"CT")
            .ok()
            .map(|value| resolve_optional(self.document, value, self.limits.max_reference_depth))
            .transpose()?
            .flatten()
            .is_some_and(|value| !matches!(value, Object::Null));
        let has_correct_alt = self.has_correct_media_clip_alt(media_clip)?;
        if media_clip_object_id.is_some_and(|id| !self.seen_media_clips.insert(id)) {
            return Ok(());
        }
        if !has_ct {
            self.summary.media_clips_missing_ct.push(RuleFailure {
                object_id: media_clip_object_id.map(Into::into),
                description: format!("{context} media clip is missing /CT"),
            });
        }
        if !has_correct_alt {
            self.summary.media_clips_missing_alt.push(RuleFailure {
                object_id: media_clip_object_id.map(Into::into),
                description: format!(
                    "{context} media clip is missing /Alt or /Alt has an incorrect value"
                ),
            });
        }
        Ok(())
    }

    // Match veraPDF 1.30.2's hasCorrectAlt predicate: /Alt is an array with
    // paired entries, and every description (the odd entry) is a non-empty
    // string. The language entries are intentionally left unconstrained
    // because veraPDF does the same.
    fn has_correct_media_clip_alt(&self, media_clip: &lopdf::Dictionary) -> Result<bool, PdfError> {
        let Some(alt) = media_clip
            .get(b"Alt")
            .ok()
            .map(|value| resolve_optional(self.document, value, self.limits.max_reference_depth))
            .transpose()?
            .flatten()
            .and_then(|value| value.as_array().ok())
        else {
            return Ok(false);
        };
        if alt.len() % 2 != 0 {
            return Ok(false);
        }
        for (index, value) in alt.iter().enumerate() {
            if index % 2 == 1 {
                let Some(value) =
                    resolve_optional(self.document, value, self.limits.max_reference_depth)?
                else {
                    return Ok(false);
                };
                if !value
                    .as_str()
                    .is_ok_and(|description| !description.is_empty())
                {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn ensure_depth(&self, depth: usize) -> Result<(), PdfError> {
        if depth > self.limits.max_reference_depth {
            Err(PdfError::ReferenceDepth(self.limits.max_reference_depth))
        } else {
            Ok(())
        }
    }
}
