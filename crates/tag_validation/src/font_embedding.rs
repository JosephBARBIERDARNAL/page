use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

use crate::content_support::{
    ContentCache, decode_content_stream_cached, for_each_page_annotation, inherited_page_resources,
    resource_once,
};
use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::object_resolution::{ResourceKey, contains_key, resolve_optional};
use crate::page_tree::PageEntry;
use crate::report::RuleFailure;

#[derive(Clone, Debug, Default)]
pub(crate) struct FontEmbeddingSummary {
    pub(crate) failures: Vec<RuleFailure>,
    pub(crate) invalid_types: Vec<RuleFailure>,
    pub(crate) invalid_subtypes: Vec<RuleFailure>,
    pub(crate) invalid_base_fonts: Vec<RuleFailure>,
    pub(crate) invalid_first_chars: Vec<RuleFailure>,
    pub(crate) invalid_last_chars: Vec<RuleFailure>,
    pub(crate) invalid_widths: Vec<RuleFailure>,
    pub(crate) invalid_font_file_subtypes: Vec<RuleFailure>,
    pub(crate) incompatible_type0_system_info: Vec<RuleFailure>,
    pub(crate) invalid_cid_to_gid_maps: Vec<RuleFailure>,
    pub(crate) unembedded_cmaps: Vec<RuleFailure>,
    pub(crate) invalid_cmap_wmodes: Vec<RuleFailure>,
    pub(crate) invalid_cmap_cids: Vec<RuleFailure>,
    pub(crate) oversized_cmap_cids: Vec<RuleFailure>,
    pub(crate) invalid_type1_subset_charsets: Vec<RuleFailure>,
    pub(crate) invalid_cid_subset_cidsets: Vec<RuleFailure>,
    pub(crate) invalid_nonsymbolic_truetype_encodings: Vec<RuleFailure>,
    pub(crate) invalid_symbolic_truetype_encodings: Vec<RuleFailure>,
    pub(crate) invalid_symbolic_truetype_cmaps: Vec<RuleFailure>,
    pub(crate) missing_truetype_glyphs: Vec<RuleFailure>,
    pub(crate) missing_type1_glyphs: Vec<RuleFailure>,
    pub(crate) inconsistent_truetype_widths: Vec<RuleFailure>,
    pub(crate) excessive_graphics_state_nesting: Vec<RuleFailure>,
}

type CidSystemInfo = (Vec<u8>, Vec<u8>);

#[derive(Clone)]
struct SelectedFont {
    key: ResourceKey,
    object: Object,
    description: String,
}

#[derive(Clone, Default)]
struct GraphicsState {
    font: Option<SelectedFont>,
    rendering_mode: i64,
}

#[derive(Clone)]
struct FontUse {
    object: Object,
    object_id: Option<PdfObjectId>,
    description: String,
    subtype: Option<String>,
    embedded: bool,
    visible: bool,
    shown_bytes: Vec<u8>,
}

struct Scanner<'a> {
    document: &'a Document,
    limits: &'a SafetyLimits,
    cache: &'a mut ContentCache,
    /// Embedded CMaps parsed once per indirect object and reused across the
    /// several checks that otherwise each independently decode and
    /// re-tokenize the same CMap stream for the same font.
    cmap_decoders: HashMap<ObjectId, Rc<CmapDecoder>>,
    uses: BTreeMap<ResourceKey, FontUse>,
    active_descendant_fonts: BTreeSet<ResourceKey>,
    /// Indirect annotation-appearance, Pattern, and Type3 CharProc streams
    /// already scanned once, so a Pattern painted repeatedly (or the same
    /// appearance stream shared by two annotations) is not re-parsed for
    /// every occurrence -- these streams take no per-invocation parameters,
    /// so a second scan can only ever repeat the first one's findings.
    scanned_standalone_streams: BTreeSet<ObjectId>,
    invalid_types: Vec<RuleFailure>,
    invalid_subtypes: Vec<RuleFailure>,
    invalid_base_fonts: Vec<RuleFailure>,
    invalid_first_chars: Vec<RuleFailure>,
    invalid_last_chars: Vec<RuleFailure>,
    invalid_widths: Vec<RuleFailure>,
    invalid_font_file_subtypes: Vec<RuleFailure>,
    incompatible_type0_system_info: Vec<RuleFailure>,
    invalid_cid_to_gid_maps: Vec<RuleFailure>,
    unembedded_cmaps: Vec<RuleFailure>,
    invalid_cmap_wmodes: Vec<RuleFailure>,
    invalid_cmap_cids: Vec<RuleFailure>,
    oversized_cmap_cids: Vec<RuleFailure>,
    invalid_type1_subset_charsets: Vec<RuleFailure>,
    invalid_cid_subset_cidsets: Vec<RuleFailure>,
    invalid_nonsymbolic_truetype_encodings: Vec<RuleFailure>,
    invalid_symbolic_truetype_encodings: Vec<RuleFailure>,
    invalid_symbolic_truetype_cmaps: Vec<RuleFailure>,
    missing_truetype_glyphs: Vec<RuleFailure>,
    missing_type1_glyphs: Vec<RuleFailure>,
    inconsistent_truetype_widths: Vec<RuleFailure>,
    excessive_graphics_state_nesting: Vec<RuleFailure>,
}

pub(crate) fn inspect(
    document: &Document,
    pages: &BTreeMap<u32, PageEntry>,
    cache: &mut ContentCache,
    limits: &SafetyLimits,
) -> Result<FontEmbeddingSummary, PdfError> {
    let mut scanner = Scanner {
        document,
        limits,
        cache,
        cmap_decoders: HashMap::new(),
        uses: BTreeMap::new(),
        active_descendant_fonts: BTreeSet::new(),
        scanned_standalone_streams: BTreeSet::new(),
        invalid_types: Vec::new(),
        invalid_subtypes: Vec::new(),
        invalid_base_fonts: Vec::new(),
        invalid_first_chars: Vec::new(),
        invalid_last_chars: Vec::new(),
        invalid_widths: Vec::new(),
        invalid_font_file_subtypes: Vec::new(),
        incompatible_type0_system_info: Vec::new(),
        invalid_cid_to_gid_maps: Vec::new(),
        unembedded_cmaps: Vec::new(),
        invalid_cmap_wmodes: Vec::new(),
        invalid_cmap_cids: Vec::new(),
        oversized_cmap_cids: Vec::new(),
        invalid_type1_subset_charsets: Vec::new(),
        invalid_cid_subset_cidsets: Vec::new(),
        invalid_nonsymbolic_truetype_encodings: Vec::new(),
        invalid_symbolic_truetype_encodings: Vec::new(),
        invalid_symbolic_truetype_cmaps: Vec::new(),
        missing_truetype_glyphs: Vec::new(),
        missing_type1_glyphs: Vec::new(),
        inconsistent_truetype_widths: Vec::new(),
        excessive_graphics_state_nesting: Vec::new(),
    };
    for (&page_number, page_entry) in pages {
        let page = page_entry
            .resolve(document)
            .ok_or(PdfError::UnexpectedObject("page is not a dictionary"))?;
        scanner.scan_page(page_number, page)?;
    }
    scanner.scan_annotation_appearances(pages)?;
    scanner.scan_type3_charproc_fonts()?;
    scanner.oversized_cmap_cids = inspect_all_embedded_cmap_cids(document, limits)?;
    scanner.invalid_cmap_cids.sort_by(|left, right| {
        left.object_id
            .cmp(&right.object_id)
            .then_with(|| left.description.cmp(&right.description))
    });
    scanner.invalid_cmap_cids.dedup_by(|left, right| {
        left.object_id == right.object_id && left.description == right.description
    });
    scanner.inspect_rendered_truetype_glyphs()?;
    scanner.inspect_rendered_type1_glyphs()?;
    scanner.inspect_rendered_type3_glyphs()?;
    scanner.inspect_rendered_cff_type1_glyphs()?;
    scanner.inspect_rendered_cidfont_glyphs()?;
    scanner.inspect_rendered_type1_subset_charsets()?;
    scanner.inspect_rendered_cid_subset_sets()?;

    let failures = scanner
        .uses
        .into_values()
        .filter(|usage| {
            usage.visible
                && !usage.embedded
                && !matches!(usage.subtype.as_deref(), Some("Type3" | "Type0"))
        })
        .map(|usage| RuleFailure {
            object_id: usage.object_id,
            description: usage.description,
        })
        .collect();
    Ok(FontEmbeddingSummary {
        failures,
        invalid_types: scanner.invalid_types,
        invalid_subtypes: scanner.invalid_subtypes,
        invalid_base_fonts: scanner.invalid_base_fonts,
        invalid_first_chars: scanner.invalid_first_chars,
        invalid_last_chars: scanner.invalid_last_chars,
        invalid_widths: scanner.invalid_widths,
        invalid_font_file_subtypes: scanner.invalid_font_file_subtypes,
        incompatible_type0_system_info: scanner.incompatible_type0_system_info,
        invalid_cid_to_gid_maps: scanner.invalid_cid_to_gid_maps,
        unembedded_cmaps: scanner.unembedded_cmaps,
        invalid_cmap_wmodes: scanner.invalid_cmap_wmodes,
        invalid_cmap_cids: scanner.invalid_cmap_cids,
        oversized_cmap_cids: scanner.oversized_cmap_cids,
        invalid_type1_subset_charsets: scanner.invalid_type1_subset_charsets,
        invalid_cid_subset_cidsets: scanner.invalid_cid_subset_cidsets,
        invalid_nonsymbolic_truetype_encodings: scanner.invalid_nonsymbolic_truetype_encodings,
        invalid_symbolic_truetype_encodings: scanner.invalid_symbolic_truetype_encodings,
        invalid_symbolic_truetype_cmaps: scanner.invalid_symbolic_truetype_cmaps,
        missing_truetype_glyphs: scanner.missing_truetype_glyphs,
        missing_type1_glyphs: scanner.missing_type1_glyphs,
        inconsistent_truetype_widths: scanner.inconsistent_truetype_widths,
        excessive_graphics_state_nesting: scanner.excessive_graphics_state_nesting,
    })
}

fn inspect_all_embedded_cmap_cids(
    document: &Document,
    limits: &SafetyLimits,
) -> Result<Vec<RuleFailure>, PdfError> {
    let mut failures = Vec::new();
    for (object_id, object) in &document.objects {
        let Ok(stream) = object.as_stream() else {
            continue;
        };
        if !stream.dict.has(b"CMapName") {
            continue;
        }
        let bytes = decode_font_stream(stream, limits)?;
        if cmap_maximal_cid(&bytes).is_some_and(|cid| cid > 65_535) {
            failures.push(font_failure(
                Some((*object_id).into()),
                &format!("embedded CMap stream {} {}", object_id.0, object_id.1),
                "contains a CID greater than 65,535",
            ));
        }
    }
    Ok(failures)
}

impl Scanner<'_> {
    fn scan_page(&mut self, page_number: u32, page: &Dictionary) -> Result<(), PdfError> {
        let resources = inherited_page_resources(self.document, page, self.limits)?;
        let mut state = GraphicsState::default();
        let mut stack = Vec::new();
        let mut active_forms = BTreeSet::new();
        let mut decoded_bytes = 0usize;
        if let Ok(contents) = page.get(b"Contents") {
            self.scan_contents(
                contents,
                resources,
                &mut state,
                &mut stack,
                &mut active_forms,
                &mut decoded_bytes,
                &format!("page {page_number}"),
                0,
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn scan_contents(
        &mut self,
        contents: &Object,
        resources: Option<&Dictionary>,
        state: &mut GraphicsState,
        stack: &mut Vec<GraphicsState>,
        active_forms: &mut BTreeSet<ObjectId>,
        decoded_bytes: &mut usize,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        if depth > self.limits.max_reference_depth {
            return Err(PdfError::ReferenceDepth(self.limits.max_reference_depth));
        }
        let content_id = contents.as_reference().ok();
        let Some(contents) =
            resolve_optional(self.document, contents, self.limits.max_reference_depth)?
        else {
            return Ok(());
        };
        if let Ok(array) = contents.as_array() {
            for item in array {
                self.scan_contents(
                    item,
                    resources,
                    state,
                    stack,
                    active_forms,
                    decoded_bytes,
                    context,
                    depth + 1,
                )?;
            }
            return Ok(());
        }
        let Ok(stream) = contents.as_stream() else {
            return Ok(());
        };
        let bytes = decode_content_stream_cached(
            stream,
            content_id,
            self.cache,
            self.limits,
            decoded_bytes,
        )?;
        let content_bytes = crate::content_support::content_without_inline_images(&bytes);
        let Ok(content) = Content::decode(&content_bytes) else {
            return Ok(());
        };
        for operation in content.operations {
            match operation.operator.as_str() {
                "q" => {
                    if stack.len() >= self.limits.max_reference_depth {
                        return Err(PdfError::ReferenceDepth(self.limits.max_reference_depth));
                    }
                    stack.push(state.clone());
                    if stack.len() > 28 {
                        self.excessive_graphics_state_nesting.push(RuleFailure {
                            object_id: None,
                            description: format!(
                                "{context} reaches graphics-state nesting depth {}",
                                stack.len()
                            ),
                        });
                    }
                }
                "Q" => {
                    if let Some(saved) = stack.pop() {
                        *state = saved;
                    }
                }
                "Tf" => {
                    let name = operation
                        .operands
                        .first()
                        .and_then(|operand| operand.as_name().ok());
                    state.font = match name {
                        Some(name) => {
                            resource_once(self.document, self.limits, resources, b"Font", name)?
                                .map(|object| SelectedFont {
                                    key: object_key(object, context, operation.operands.first()),
                                    object: object.clone(),
                                    description: describe_font(
                                        object,
                                        context,
                                        operation.operands.first(),
                                    ),
                                })
                        }
                        None => None,
                    };
                }
                "Tr" => {
                    if let Some(mode) = operation
                        .operands
                        .first()
                        .and_then(|operand| operand.as_i64().ok())
                    {
                        state.rendering_mode = mode;
                    }
                }
                "Tj" | "TJ" | "'" | "\"" if shows_text(&operation.operands) => {
                    if let Some(font) = state.font.clone() {
                        self.record_font(
                            &font,
                            state.rendering_mode,
                            &shown_text_bytes(&operation.operands),
                        )?;
                    }
                }
                "Do" => {
                    let Some(name) = operation
                        .operands
                        .first()
                        .and_then(|operand| operand.as_name().ok())
                    else {
                        continue;
                    };
                    let Some(object) =
                        resource_once(self.document, self.limits, resources, b"XObject", name)?
                    else {
                        continue;
                    };
                    let form_id = object.as_reference().ok();
                    if form_id.is_some_and(|id| !active_forms.insert(id)) {
                        continue;
                    }
                    let result = self.scan_form(
                        object,
                        resources,
                        state,
                        active_forms,
                        decoded_bytes,
                        &format!("{context}/Form /{}", String::from_utf8_lossy(name)),
                        depth + 1,
                    );
                    if let Some(id) = form_id {
                        active_forms.remove(&id);
                    }
                    result?;
                }
                "scn" | "SCN" => {
                    let Some(name) = operation
                        .operands
                        .last()
                        .and_then(|operand| operand.as_name().ok())
                    else {
                        continue;
                    };
                    let Some(pattern) =
                        resource_once(self.document, self.limits, resources, b"Pattern", name)?
                    else {
                        continue;
                    };
                    self.scan_form_like_stream_once(
                        pattern,
                        None,
                        &format!("{context}/Pattern /{}", String::from_utf8_lossy(name)),
                    )?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn scan_form(
        &mut self,
        object: &Object,
        parent_resources: Option<&Dictionary>,
        state: &GraphicsState,
        active_forms: &mut BTreeSet<ObjectId>,
        decoded_bytes: &mut usize,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        let Some(form) = resolve_optional(self.document, object, self.limits.max_reference_depth)?
            .and_then(|object| object.as_stream().ok())
        else {
            return Ok(());
        };
        if resolved_name(self.document, &form.dict, b"Subtype", self.limits)?
            != Some(b"Form".as_slice())
        {
            return Ok(());
        }
        let resources = match form.dict.get(b"Resources") {
            Ok(entry) => resolve_optional(self.document, entry, self.limits.max_reference_depth)?
                .and_then(|object| object.as_dict().ok()),
            Err(_) => parent_resources,
        };
        let mut form_state = state.clone();
        let mut stack = Vec::new();
        self.scan_contents(
            object,
            resources,
            &mut form_state,
            &mut stack,
            active_forms,
            decoded_bytes,
            context,
            depth,
        )
    }

    /// Scans every page annotation's `/AP` `/N` appearance stream for font
    /// use, confirmed live against veraPDF 1.28.2: a font referenced only
    /// from an annotation appearance (never from the page's own content)
    /// still populates a `PDFont` object and is still checked for embedding.
    /// A button Widget's `/N` is a subdictionary of named states (e.g.
    /// `Off`/`Yes`); veraPDF walks every state's stream, not just the one
    /// selected by `/AS` (also confirmed live), so every value is scanned.
    fn scan_annotation_appearances(
        &mut self,
        pages: &BTreeMap<u32, PageEntry>,
    ) -> Result<(), PdfError> {
        let mut inspected = BTreeSet::new();
        let mut annotations = Vec::new();
        for_each_page_annotation(
            self.document,
            pages,
            self.limits,
            &mut inspected,
            |page_number, index, _object_id, annotation| {
                if let Ok(dictionary) = annotation.as_dict() {
                    annotations.push((page_number, index, dictionary.clone()));
                }
                Ok(())
            },
        )?;
        for (page_number, index, annotation) in annotations {
            let Ok(appearance) = annotation.get(b"AP") else {
                continue;
            };
            let Some(appearance) =
                resolve_optional(self.document, appearance, self.limits.max_reference_depth)?
                    .and_then(|object| object.as_dict().ok())
            else {
                continue;
            };
            // Confirmed live against veraPDF 1.28.2: /D (down) and /R
            // (rollover) appearance streams are walked for font use exactly
            // like /N, even though their mere presence already fails
            // PDFA1B-ANNOTATION-AP-ENTRIES-001 on its own (a compliant file
            // never has them, but veraPDF still evaluates every other
            // predicate against the same object graph regardless).
            for key in [b"N".as_slice(), b"D".as_slice(), b"R".as_slice()] {
                let Ok(entry) = appearance.get(key) else {
                    continue;
                };
                let Some(resolved_entry) =
                    resolve_optional(self.document, entry, self.limits.max_reference_depth)?
                else {
                    continue;
                };
                let context = format!(
                    "page {page_number} annotation {index}/AP/{}",
                    String::from_utf8_lossy(key)
                );
                if resolved_entry.as_stream().is_ok() {
                    self.scan_form_like_stream_once(entry, None, &context)?;
                } else if let Ok(states) = resolved_entry.as_dict() {
                    for (state, value) in states.iter() {
                        let state_context = format!("{context}/{}", String::from_utf8_lossy(state));
                        self.scan_form_like_stream_once(value, None, &state_context)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Scans a Pattern or Type3 CharProc content stream (or an annotation
    /// appearance stream) exactly once, deduplicating indirect streams via
    /// `scanned_standalone_streams` -- unlike a page/Form invocation, these
    /// streams take no per-invocation parameters, so a repeat scan can only
    /// ever repeat the first scan's findings.
    fn scan_form_like_stream_once(
        &mut self,
        object: &Object,
        fallback_resources: Option<&Dictionary>,
        context: &str,
    ) -> Result<(), PdfError> {
        if let Ok(id) = object.as_reference()
            && !self.scanned_standalone_streams.insert(id)
        {
            return Ok(());
        }
        self.scan_form_like_stream(object, fallback_resources, context, 0)
    }

    /// Executes a standalone content stream that is shaped like a Form
    /// XObject (its own optional `/Resources`, otherwise `fallback_resources`)
    /// but is reached by a route other than a page/Form `Do` invocation: an
    /// annotation appearance, a Pattern's content, or a Type3 glyph
    /// CharProc. Reuses `scan_contents` unchanged, so every existing font
    /// check (embedding, widths, CMaps, glyph presence, ...) applies to
    /// fonts discovered this way exactly as it does for page/Form fonts.
    fn scan_form_like_stream(
        &mut self,
        object: &Object,
        fallback_resources: Option<&Dictionary>,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        let Some(resolved) =
            resolve_optional(self.document, object, self.limits.max_reference_depth)?
        else {
            return Ok(());
        };
        let Ok(stream) = resolved.as_stream() else {
            return Ok(());
        };
        let resources = match stream.dict.get(b"Resources") {
            Ok(entry) => resolve_optional(self.document, entry, self.limits.max_reference_depth)?
                .and_then(|object| object.as_dict().ok()),
            Err(_) => fallback_resources,
        };
        let mut state = GraphicsState::default();
        let mut stack = Vec::new();
        let mut active_forms = BTreeSet::new();
        let mut decoded_bytes = 0usize;
        self.scan_contents(
            object,
            resources,
            &mut state,
            &mut stack,
            &mut active_forms,
            &mut decoded_bytes,
            context,
            depth,
        )
    }

    /// Scans the `/CharProcs` glyph stream for every byte actually rendered
    /// through a used Type3 font, confirmed live against veraPDF 1.28.2 to
    /// populate a `PDFont` object for a font shown only from inside a Type3
    /// glyph description. Falls back to the Type3 font's own `/Resources`
    /// per PDF32000 9.6.5.2 when a CharProc stream has none of its own.
    /// Loops to a fixed point (bounded by `max_reference_depth` rounds)
    /// since scanning a CharProc can itself discover a further Type3 font
    /// whose own CharProcs have not yet been scanned.
    fn scan_type3_charproc_fonts(&mut self) -> Result<(), PdfError> {
        for _ in 0..=self.limits.max_reference_depth {
            let before = self.uses.len();
            let uses: Vec<_> = self.uses.values().cloned().collect();
            for usage in uses {
                if !usage.visible
                    || usage.subtype.as_deref() != Some("Type3")
                    || usage.shown_bytes.is_empty()
                {
                    continue;
                }
                let Some(object) = resolve_optional(
                    self.document,
                    &usage.object,
                    self.limits.max_reference_depth,
                )?
                else {
                    continue;
                };
                let Ok(font) = object.as_dict() else {
                    continue;
                };
                let Some(char_procs) = font
                    .get(b"CharProcs")
                    .ok()
                    .map(|value| {
                        resolve_optional(self.document, value, self.limits.max_reference_depth)
                    })
                    .transpose()?
                    .flatten()
                    .and_then(|value| value.as_dict().ok())
                else {
                    continue;
                };
                let differences = type1_encoding_differences(self.document, font, self.limits)?;
                let fallback_resources = font
                    .get(b"Resources")
                    .ok()
                    .map(|value| {
                        resolve_optional(self.document, value, self.limits.max_reference_depth)
                    })
                    .transpose()?
                    .flatten()
                    .and_then(|value| value.as_dict().ok())
                    .cloned();
                for byte in usage.shown_bytes.iter().copied().collect::<BTreeSet<_>>() {
                    let Some(name) = differences.get(&byte) else {
                        continue;
                    };
                    let Ok(charproc) = char_procs.get(name.as_bytes()) else {
                        continue;
                    };
                    let context = format!("{}/CharProc /{name}", usage.description);
                    self.scan_form_like_stream_once(
                        charproc,
                        fallback_resources.as_ref(),
                        &context,
                    )?;
                }
            }
            if self.uses.len() == before {
                return Ok(());
            }
        }
        Err(PdfError::ReferenceDepth(self.limits.max_reference_depth))
    }

    fn record_font(
        &mut self,
        selected: &SelectedFont,
        rendering_mode: i64,
        shown_bytes: &[u8],
    ) -> Result<(), PdfError> {
        if let Some(font_use) = self.uses.get_mut(&selected.key) {
            if rendering_mode != 3 {
                font_use.shown_bytes.extend_from_slice(shown_bytes);
            }
            return Ok(());
        }
        let Some(object) = resolve_optional(
            self.document,
            &selected.object,
            self.limits.max_reference_depth,
        )?
        else {
            return Ok(());
        };
        let Ok(font) = object.as_dict() else {
            return Ok(());
        };
        let subtype = resolved_name(self.document, font, b"Subtype", self.limits)?
            .map(|value| String::from_utf8_lossy(value).into_owned());
        // veraPDF 1.28.2 does not create a PDFont model object for a missing
        // or unsupported subtype, so none of the PDFont predicates are
        // instantiated for such a resource.
        if !matches!(
            subtype.as_deref(),
            Some(
                "Type1"
                    | "MMType1"
                    | "TrueType"
                    | "Type3"
                    | "Type0"
                    | "CIDFontType0"
                    | "CIDFontType2"
            )
        ) {
            return Ok(());
        }
        let object_id = selected.key.object_id();
        self.inspect_font_dictionary(font, object_id, &selected.description, subtype.as_deref())?;
        if subtype.as_deref() == Some("Type0") {
            self.inspect_composite_font(font, object_id, &selected.description, rendering_mode)?;
        } else if subtype.as_deref() == Some("TrueType") {
            self.inspect_truetype_font(font, object_id, &selected.description)?;
        } else if subtype.as_deref() == Some("Type1") {
            self.inspect_type1_subset_font(font, object_id, &selected.description)?;
        }
        let embedded =
            if rendering_mode == 3 || matches!(subtype.as_deref(), Some("Type3" | "Type0")) {
                false
            } else {
                font_is_embedded(self.document, font, self.limits)?
            };
        self.uses.entry(selected.key.clone()).or_insert(FontUse {
            object: selected.object.clone(),
            object_id,
            description: selected.description.clone(),
            subtype: subtype.clone(),
            embedded,
            // veraPDF 1.28.2 associates the first observed rendering mode
            // with a font model object and does not revise it on later uses.
            visible: rendering_mode != 3,
            shown_bytes: if rendering_mode == 3 {
                Vec::new()
            } else {
                shown_bytes.to_vec()
            },
        });

        if subtype.as_deref() == Some("Type0")
            && self.active_descendant_fonts.insert(selected.key.clone())
        {
            // PDF32000 9.7.3 requires exactly one entry, and veraPDF 1.28.2
            // confirms this in its object model: it creates a PDCIDFont (and
            // evaluates every per-font predicate, including embedding) only
            // for DescendantFonts[0]. A second or later entry is invisible
            // to it entirely, so recording it here too would make a local
            // check fail (e.g. embedding) for an object veraPDF never
            // examines -- a confirmed false positive, not extra coverage.
            if let Ok(descendants) = font.get(b"DescendantFonts")
                && let Some(descendants) =
                    resolve_optional(self.document, descendants, self.limits.max_reference_depth)?
                        .and_then(|object| object.as_array().ok())
                && let Some(descendant) = descendants.first()
            {
                let context = format!("{}/descendant 0", selected.description);
                let descendant = SelectedFont {
                    key: object_key(descendant, &context, None),
                    object: descendant.clone(),
                    description: describe_descendant(descendant, &selected.description),
                };
                self.record_font(&descendant, rendering_mode, &[])?;
            }
            self.active_descendant_fonts.remove(&selected.key);
        }
        Ok(())
    }

    /// Resolves the CIDs a Type0 font's `encoding` maps `shown_bytes`
    /// through, reusing a previously decoded/parsed embedded CMap for the
    /// same indirect object instead of re-decompressing and re-tokenizing it.
    /// The two glyph-presence and width checks that need this both look it
    /// up for the same font, so a cache hit is the common case.
    fn cached_cids_for_rendered_bytes(
        &mut self,
        encoding: &Object,
        shown_bytes: &[u8],
    ) -> Result<Option<Vec<u16>>, PdfError> {
        let Some(object_id) = encoding.as_reference().ok() else {
            let decoder = resolve_cmap_decoder(self.document, encoding, self.limits)?;
            return Ok(decoder.decode(shown_bytes));
        };
        let decoder = match self.cmap_decoders.get(&object_id) {
            Some(decoder) => Rc::clone(decoder),
            None => {
                let decoder = Rc::new(resolve_cmap_decoder(self.document, encoding, self.limits)?);
                self.cmap_decoders.insert(object_id, Rc::clone(&decoder));
                decoder
            }
        };
        Ok(decoder.decode(shown_bytes))
    }

    fn inspect_truetype_font(
        &mut self,
        font: &Dictionary,
        object_id: Option<PdfObjectId>,
        description: &str,
    ) -> Result<(), PdfError> {
        let descriptor = font_descriptor_dictionary(self.document, font, self.limits)?;
        let symbolic = descriptor
            .and_then(|descriptor| descriptor.get(b"Flags").ok())
            .map(|value| resolved_integer(self.document, self.limits, value))
            .transpose()?
            .flatten()
            .is_some_and(|flags| flags & 4 != 0);
        let (encoding, contains_differences) = truetype_encoding(self.document, font, self.limits)?;

        if symbolic {
            if encoding.is_some() {
                self.invalid_symbolic_truetype_encodings.push(font_failure(
                    object_id,
                    description,
                    "is symbolic but specifies an /Encoding",
                ));
            }
            if let Some(cmap_count) = truetype_cmap_count(self.document, descriptor, self.limits)?
                && cmap_count != 1
            {
                self.invalid_symbolic_truetype_cmaps.push(font_failure(
                    object_id,
                    description,
                    &format!(
                        "is symbolic but its embedded TrueType program contains {cmap_count} cmap subtables"
                    ),
                ));
            }
        } else if !matches!(
            encoding.as_deref(),
            Some(b"MacRomanEncoding" | b"WinAnsiEncoding")
        ) || contains_differences
        {
            self.invalid_nonsymbolic_truetype_encodings
                .push(font_failure(
                    object_id,
                    description,
                    "is non-symbolic but lacks an unmodified MacRomanEncoding or WinAnsiEncoding",
                ));
        }
        Ok(())
    }

    fn inspect_rendered_truetype_glyphs(&mut self) -> Result<(), PdfError> {
        let uses: Vec<_> = self.uses.values().cloned().collect();
        for usage in uses {
            if !usage.visible
                || !usage.embedded
                || usage.subtype.as_deref() != Some("TrueType")
                || usage.shown_bytes.is_empty()
            {
                continue;
            }
            let Some(object) = resolve_optional(
                self.document,
                &usage.object,
                self.limits.max_reference_depth,
            )?
            else {
                continue;
            };
            let Ok(font) = object.as_dict() else {
                continue;
            };
            let (encoding, contains_differences) =
                truetype_encoding(self.document, font, self.limits)?;
            if !matches!(
                encoding.as_deref(),
                Some(b"MacRomanEncoding" | b"WinAnsiEncoding")
            ) || contains_differences
            {
                continue;
            }
            let Some(descriptor) = font_descriptor_dictionary(self.document, font, self.limits)?
            else {
                continue;
            };
            let Ok(file) = descriptor.get(b"FontFile2") else {
                continue;
            };
            let Some(stream) =
                resolve_optional(self.document, file, self.limits.max_reference_depth)?
                    .and_then(|object| object.as_stream().ok())
            else {
                continue;
            };
            let bytes = decode_font_stream(stream, self.limits)?;
            let Ok(face) = ttf_parser::Face::parse(&bytes, 0) else {
                continue;
            };
            let first_char = font
                .get(b"FirstChar")
                .ok()
                .map(|value| resolved_integer(self.document, self.limits, value))
                .transpose()?
                .flatten();
            let widths = resolved_array(self.document, font, b"Widths", self.limits)?;
            let Some(encoding) = encoding.as_deref() else {
                continue;
            };
            let mut encoding_dictionary = Dictionary::new();
            encoding_dictionary.set("Type", "Font");
            encoding_dictionary.set("Encoding", Object::Name(encoding.to_vec()));
            let Ok(pdf_encoding) = encoding_dictionary.get_font_encoding(self.document) else {
                continue;
            };
            for byte in usage.shown_bytes.into_iter().collect::<BTreeSet<_>>() {
                let Some(character) = single_encoded_character(&pdf_encoding, byte) else {
                    continue;
                };
                let Some(glyph) = face.glyph_index(character) else {
                    self.missing_truetype_glyphs.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        &format!("has no embedded TrueType glyph for rendered byte {byte}"),
                    ));
                    continue;
                };
                let (Some(first_char), Some(widths), Some(advance)) =
                    (first_char, widths, face.glyph_hor_advance(glyph))
                else {
                    continue;
                };
                let Some(index) = i64::from(byte).checked_sub(first_char) else {
                    continue;
                };
                let Ok(index) = usize::try_from(index) else {
                    continue;
                };
                let Some(dictionary_width) = widths
                    .get(index)
                    .map(|value| resolved_float(self.document, self.limits, value))
                    .transpose()?
                    .flatten()
                else {
                    continue;
                };
                let program_width = f64::from(advance) * 1000.0 / f64::from(face.units_per_em());
                if (program_width - f64::from(dictionary_width)).abs() > 1.0 {
                    self.inconsistent_truetype_widths.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        &format!(
                            "has rendered byte {byte} width {program_width:.3} in its embedded TrueType program but {dictionary_width:.3} in /Widths"
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    fn inspect_rendered_cidfont_glyphs(&mut self) -> Result<(), PdfError> {
        let uses: Vec<_> = self.uses.values().cloned().collect();
        for usage in uses {
            if !usage.visible
                || usage.subtype.as_deref() != Some("Type0")
                || usage.shown_bytes.is_empty()
            {
                continue;
            }
            let Some(object) = resolve_optional(
                self.document,
                &usage.object,
                self.limits.max_reference_depth,
            )?
            else {
                continue;
            };
            let Ok(font) = object.as_dict() else {
                continue;
            };
            let Ok(encoding) = font.get(b"Encoding") else {
                continue;
            };
            let Some(cids) = self.cached_cids_for_rendered_bytes(encoding, &usage.shown_bytes)?
            else {
                continue;
            };
            let Some(descendant) = first_descendant_dictionary(self.document, font, self.limits)?
            else {
                continue;
            };
            let descendant_subtype =
                resolved_name(self.document, descendant, b"Subtype", self.limits)?;
            if descendant_subtype == Some(b"CIDFontType0".as_slice()) {
                self.inspect_rendered_cff_cidfont_glyphs(&usage, descendant, &cids)?;
                continue;
            }
            if descendant_subtype != Some(b"CIDFontType2".as_slice()) {
                continue;
            }
            let Ok(cid_to_gid) = descendant.get(b"CIDToGIDMap") else {
                continue;
            };
            let Some(descriptor) =
                font_descriptor_dictionary(self.document, descendant, self.limits)?
            else {
                continue;
            };
            let Some(stream) = descriptor
                .get(b"FontFile2")
                .ok()
                .map(|value| {
                    resolve_optional(self.document, value, self.limits.max_reference_depth)
                })
                .transpose()?
                .flatten()
                .and_then(|value| value.as_stream().ok())
            else {
                continue;
            };
            let bytes = decode_font_stream(stream, self.limits)?;
            let Ok(face) = ttf_parser::Face::parse(&bytes, 0) else {
                continue;
            };
            // Resolved/parsed once per font instead of once per rendered CID.
            let cid_to_gid_map = resolve_cid_to_gid_map(self.document, cid_to_gid, self.limits)?;
            let cid_widths = parse_cid_widths(self.document, descendant, self.limits)?;
            for cid in cids.into_iter().collect::<BTreeSet<_>>() {
                if cid == 0 {
                    continue;
                }
                let Some(glyph) = cid_to_gid_map.glyph_for(cid) else {
                    continue;
                };
                let Some(advance) = face.glyph_hor_advance(glyph) else {
                    self.missing_truetype_glyphs.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        &format!("has no embedded CIDFontType2 glyph for rendered CID {cid}"),
                    ));
                    continue;
                };
                let Some(dictionary_width) = cid_widths.width_for(cid) else {
                    continue;
                };
                let program_width = f64::from(advance) * 1000.0 / f64::from(face.units_per_em());
                if (program_width - dictionary_width).abs() > 1.0 {
                    self.inconsistent_truetype_widths.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        &format!(
                            "has rendered CID {cid} width {program_width:.3} in its embedded CIDFontType2 program but {dictionary_width:.3} in /DW"
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    fn inspect_rendered_cff_cidfont_glyphs(
        &mut self,
        usage: &FontUse,
        descendant: &Dictionary,
        cids: &[u16],
    ) -> Result<(), PdfError> {
        let Some(descriptor) = font_descriptor_dictionary(self.document, descendant, self.limits)?
        else {
            return Ok(());
        };
        let Some(stream) = descriptor
            .get(b"FontFile3")
            .ok()
            .map(|value| resolve_optional(self.document, value, self.limits.max_reference_depth))
            .transpose()?
            .flatten()
            .and_then(|value| value.as_stream().ok())
        else {
            return Ok(());
        };
        if resolved_name(self.document, &stream.dict, b"Subtype", self.limits)?
            != Some(b"CIDFontType0C".as_slice())
        {
            return Ok(());
        }
        let bytes = decode_font_stream(stream, self.limits)?;
        let Some(cff) = ttf_parser::cff::Table::parse(&bytes) else {
            return Ok(());
        };
        // Built once per font instead of scanning every glyph per rendered
        // CID; `or_insert` keeps the same lowest-glyph-id tie-break as the
        // original `.find()` over an ascending glyph-id range.
        let mut glyph_by_cid: HashMap<u16, ttf_parser::GlyphId> = HashMap::new();
        for glyph in (0..cff.number_of_glyphs()).map(ttf_parser::GlyphId) {
            if let Some(cid) = cff.glyph_cid(glyph) {
                glyph_by_cid.entry(cid).or_insert(glyph);
            }
        }
        let cid_widths = parse_cid_widths(self.document, descendant, self.limits)?;
        for cid in cids.iter().copied().collect::<BTreeSet<_>>() {
            if cid == 0 {
                continue;
            }
            let Some(&glyph) = glyph_by_cid.get(&cid) else {
                self.missing_truetype_glyphs.push(font_failure(
                    usage.object_id,
                    &usage.description,
                    &format!("has no embedded CIDFontType0C glyph for rendered CID {cid}"),
                ));
                continue;
            };
            let Some(program_width) = cff_cid_glyph_width(&bytes, glyph.0) else {
                continue;
            };
            let Some(dictionary_width) = cid_widths.width_for(cid) else {
                continue;
            };
            if (program_width - dictionary_width).abs() > 1.0 {
                self.inconsistent_truetype_widths.push(font_failure(
                    usage.object_id,
                    &usage.description,
                    &format!(
                        "has rendered CID {cid} width {program_width:.3} in its embedded CIDFontType0C program but {dictionary_width:.3} in /W or /DW"
                    ),
                ));
            }
        }
        Ok(())
    }

    fn inspect_rendered_type1_subset_charsets(&mut self) -> Result<(), PdfError> {
        let uses: Vec<_> = self.uses.values().cloned().collect();
        for usage in uses {
            if !usage.visible
                || !usage.embedded
                || usage.subtype.as_deref() != Some("Type1")
                || usage.shown_bytes.is_empty()
            {
                continue;
            }
            let Some(object) = resolve_optional(
                self.document,
                &usage.object,
                self.limits.max_reference_depth,
            )?
            else {
                continue;
            };
            let Ok(font) = object.as_dict() else {
                continue;
            };
            if !resolved_name(self.document, font, b"BaseFont", self.limits)?
                .is_some_and(is_subset_font_name)
            {
                continue;
            }
            let Some(descriptor) = font_descriptor_dictionary(self.document, font, self.limits)?
            else {
                continue;
            };
            let Some(char_set_bytes) =
                resolved_string(self.document, descriptor, b"CharSet", self.limits)?
            else {
                continue;
            };
            let char_set = type1_charset_names(&char_set_bytes);
            let Some(stream) = descriptor
                .get(b"FontFile")
                .ok()
                .map(|value| {
                    resolve_optional(self.document, value, self.limits.max_reference_depth)
                })
                .transpose()?
                .flatten()
                .and_then(|value| value.as_stream().ok())
            else {
                continue;
            };
            let program_names = type1_program_char_names(&decode_font_stream(stream, self.limits)?);
            if program_names.is_empty() {
                continue;
            }
            let differences = type1_encoding_differences(self.document, font, self.limits)?;
            if usage.shown_bytes.into_iter().any(|byte| {
                let name = differences
                    .get(&byte)
                    .map(String::as_str)
                    .or_else(|| type1_standard_glyph_name(byte));
                name.is_some_and(|name| program_names.contains(name) && !char_set.contains(name))
            }) {
                self.invalid_type1_subset_charsets.push(font_failure(
                    usage.object_id,
                    &usage.description,
                    "has a rendered Type1 glyph absent from its descriptor /CharSet",
                ));
            }
        }
        Ok(())
    }

    fn inspect_rendered_type1_glyphs(&mut self) -> Result<(), PdfError> {
        let uses: Vec<_> = self.uses.values().cloned().collect();
        for usage in uses {
            if !usage.visible
                || !usage.embedded
                || usage.subtype.as_deref() != Some("Type1")
                || usage.shown_bytes.is_empty()
            {
                continue;
            }
            let Some(object) = resolve_optional(
                self.document,
                &usage.object,
                self.limits.max_reference_depth,
            )?
            else {
                continue;
            };
            let Ok(font) = object.as_dict() else {
                continue;
            };
            let Some(descriptor) = font_descriptor_dictionary(self.document, font, self.limits)?
            else {
                continue;
            };
            let Some(stream) = descriptor
                .get(b"FontFile")
                .ok()
                .map(|value| {
                    resolve_optional(self.document, value, self.limits.max_reference_depth)
                })
                .transpose()?
                .flatten()
                .and_then(|value| value.as_stream().ok())
            else {
                continue;
            };
            let program_bytes = decode_font_stream(stream, self.limits)?;
            let program_names = type1_program_char_names(&program_bytes);
            let program_widths = type1_program_charstring_widths(&program_bytes);
            if program_names.is_empty() {
                continue;
            }
            let differences = type1_encoding_differences(self.document, font, self.limits)?;
            let first_char = font
                .get(b"FirstChar")
                .ok()
                .map(|value| resolved_integer(self.document, self.limits, value))
                .transpose()?
                .flatten();
            let widths = resolved_array(self.document, font, b"Widths", self.limits)?;
            for byte in usage.shown_bytes.into_iter().collect::<BTreeSet<_>>() {
                let Some(name) = differences
                    .get(&byte)
                    .map(String::as_str)
                    .or_else(|| type1_standard_glyph_name(byte))
                else {
                    continue;
                };
                if !program_names.contains(name) {
                    self.missing_type1_glyphs.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        &format!("has no embedded Type1 glyph for rendered byte {byte}"),
                    ));
                    continue;
                }
                let (Some(program_width), Some(first_char), Some(widths)) =
                    (program_widths.get(name), first_char, widths)
                else {
                    continue;
                };
                let Some(index) = i64::from(byte).checked_sub(first_char) else {
                    continue;
                };
                let Ok(index) = usize::try_from(index) else {
                    continue;
                };
                let Some(dictionary_width) = widths
                    .get(index)
                    .map(|value| resolved_float(self.document, self.limits, value))
                    .transpose()?
                    .flatten()
                else {
                    continue;
                };
                if (*program_width - f64::from(dictionary_width)).abs() > 1.0 {
                    self.inconsistent_truetype_widths.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        &format!(
                            "has rendered byte {byte} width {program_width:.3} in its embedded Type1 program but {dictionary_width:.3} in /Widths"
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    fn inspect_rendered_type3_glyphs(&mut self) -> Result<(), PdfError> {
        let uses: Vec<_> = self.uses.values().cloned().collect();
        for usage in uses {
            if !usage.visible
                || usage.subtype.as_deref() != Some("Type3")
                || usage.shown_bytes.is_empty()
            {
                continue;
            }
            let Some(object) = resolve_optional(
                self.document,
                &usage.object,
                self.limits.max_reference_depth,
            )?
            else {
                continue;
            };
            let Ok(font) = object.as_dict() else {
                continue;
            };
            let Some(char_procs) = font
                .get(b"CharProcs")
                .ok()
                .map(|value| {
                    resolve_optional(self.document, value, self.limits.max_reference_depth)
                })
                .transpose()?
                .flatten()
                .and_then(|value| value.as_dict().ok())
            else {
                continue;
            };
            let differences = type1_encoding_differences(self.document, font, self.limits)?;
            if usage.shown_bytes.into_iter().any(|byte| {
                differences
                    .get(&byte)
                    .map(String::as_bytes)
                    .is_some_and(|name| !char_procs.has(name))
            }) {
                self.missing_type1_glyphs.push(font_failure(
                    usage.object_id,
                    &usage.description,
                    "has a rendered Type3 glyph absent from /CharProcs",
                ));
                self.inconsistent_truetype_widths.push(font_failure(
                    usage.object_id,
                    &usage.description,
                    "has a rendered Type3 glyph without a /CharProcs width declaration",
                ));
            }
        }
        Ok(())
    }

    fn inspect_rendered_cff_type1_glyphs(&mut self) -> Result<(), PdfError> {
        let uses: Vec<_> = self.uses.values().cloned().collect();
        for usage in uses {
            if !usage.visible
                || !usage.embedded
                || !matches!(usage.subtype.as_deref(), Some("Type1" | "MMType1"))
                || usage.shown_bytes.is_empty()
            {
                continue;
            }
            let Some(object) = resolve_optional(
                self.document,
                &usage.object,
                self.limits.max_reference_depth,
            )?
            else {
                continue;
            };
            let Ok(font) = object.as_dict() else {
                continue;
            };
            let Some(descriptor) = font_descriptor_dictionary(self.document, font, self.limits)?
            else {
                continue;
            };
            let Some(stream) = descriptor
                .get(b"FontFile3")
                .ok()
                .map(|value| {
                    resolve_optional(self.document, value, self.limits.max_reference_depth)
                })
                .transpose()?
                .flatten()
                .and_then(|value| value.as_stream().ok())
            else {
                continue;
            };
            if resolved_name(self.document, &stream.dict, b"Subtype", self.limits)?
                != Some(b"Type1C".as_slice())
            {
                continue;
            }
            let bytes = decode_font_stream(stream, self.limits)?;
            let Some(cff) = ttf_parser::cff::Table::parse(&bytes) else {
                continue;
            };
            let differences = type1_encoding_differences(self.document, font, self.limits)?;
            let first_char = font
                .get(b"FirstChar")
                .ok()
                .map(|value| resolved_integer(self.document, self.limits, value))
                .transpose()?
                .flatten();
            let widths = resolved_array(self.document, font, b"Widths", self.limits)?;
            for byte in usage.shown_bytes.into_iter().collect::<BTreeSet<_>>() {
                let Some(name) = differences
                    .get(&byte)
                    .map(String::as_str)
                    .or_else(|| type1_standard_glyph_name(byte))
                else {
                    continue;
                };
                let Some(glyph) = cff.glyph_index_by_name(name) else {
                    self.missing_type1_glyphs.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        &format!("has no embedded Type1C glyph for rendered byte {byte}"),
                    ));
                    continue;
                };
                let (Some(first_char), Some(widths), Some(width)) =
                    (first_char, widths, cff.glyph_width(glyph))
                else {
                    continue;
                };
                let Some(index) = i64::from(byte).checked_sub(first_char) else {
                    continue;
                };
                let Ok(index) = usize::try_from(index) else {
                    continue;
                };
                let Some(dictionary_width) = widths
                    .get(index)
                    .map(|value| resolved_float(self.document, self.limits, value))
                    .transpose()?
                    .flatten()
                else {
                    continue;
                };
                let program_width = f64::from(width) * f64::from(cff.matrix().sx) * 1000.0;
                if (program_width - f64::from(dictionary_width)).abs() > 1.0 {
                    self.inconsistent_truetype_widths.push(font_failure(
                        usage.object_id,
                        &usage.description,
                        &format!(
                            "has rendered byte {byte} width {program_width:.3} in its embedded Type1C program but {dictionary_width:.3} in /Widths"
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    fn inspect_rendered_cid_subset_sets(&mut self) -> Result<(), PdfError> {
        let uses: Vec<_> = self.uses.values().cloned().collect();
        for usage in uses {
            if !usage.visible
                || usage.subtype.as_deref() != Some("Type0")
                || usage.shown_bytes.is_empty()
            {
                continue;
            }
            let Some(object) = resolve_optional(
                self.document,
                &usage.object,
                self.limits.max_reference_depth,
            )?
            else {
                continue;
            };
            let Ok(font) = object.as_dict() else {
                continue;
            };
            let Ok(encoding) = font.get(b"Encoding") else {
                continue;
            };
            let Some(cids) = self.cached_cids_for_rendered_bytes(encoding, &usage.shown_bytes)?
            else {
                continue;
            };
            let Some(descendant) = first_descendant_dictionary(self.document, font, self.limits)?
            else {
                continue;
            };
            let descendant_subtype =
                resolved_name(self.document, descendant, b"Subtype", self.limits)?;
            if !matches!(descendant_subtype, Some(b"CIDFontType2" | b"CIDFontType0"))
                || !resolved_name(self.document, descendant, b"BaseFont", self.limits)?
                    .is_some_and(is_subset_font_name)
            {
                continue;
            }
            let Some(descriptor) =
                font_descriptor_dictionary(self.document, descendant, self.limits)?
            else {
                continue;
            };
            let Some(cid_set) = descriptor
                .get(b"CIDSet")
                .ok()
                .map(|value| {
                    resolve_optional(self.document, value, self.limits.max_reference_depth)
                })
                .transpose()?
                .flatten()
                .and_then(|value| value.as_stream().ok())
            else {
                continue;
            };
            let cid_set_bytes = decode_font_stream(cid_set, self.limits)?;
            if !cids
                .into_iter()
                .any(|cid| cid != 0 && !cid_set_contains(&cid_set_bytes, cid))
            {
                continue;
            }
            self.invalid_cid_subset_cidsets.push(font_failure(
                usage.object_id,
                &usage.description,
                "has a rendered CID absent from its descriptor /CIDSet",
            ));
        }
        Ok(())
    }

    fn inspect_type1_subset_font(
        &mut self,
        font: &Dictionary,
        object_id: Option<PdfObjectId>,
        description: &str,
    ) -> Result<(), PdfError> {
        let is_subset = resolved_name(self.document, font, b"BaseFont", self.limits)?
            .is_some_and(is_subset_font_name);
        if !is_subset {
            return Ok(());
        }
        let has_charset = font_descriptor_dictionary(self.document, font, self.limits)?
            .map(|descriptor| resolved_string(self.document, descriptor, b"CharSet", self.limits))
            .transpose()?
            .flatten()
            .is_some();
        if !has_charset {
            self.invalid_type1_subset_charsets.push(font_failure(
                object_id,
                description,
                "is a Type1 subset without a descriptor /CharSet string",
            ));
        }
        Ok(())
    }

    fn inspect_composite_font(
        &mut self,
        font: &Dictionary,
        object_id: Option<PdfObjectId>,
        description: &str,
        rendering_mode: i64,
    ) -> Result<(), PdfError> {
        let descendant = first_descendant_dictionary(self.document, font, self.limits)?;
        if let Some(descendant) = descendant {
            self.inspect_cid_subset_font(descendant, object_id, description)?;
        }
        if let Some(descendant) = descendant
            && resolved_name(self.document, descendant, b"Subtype", self.limits)?
                == Some(b"CIDFontType2".as_slice())
            && rendering_mode != 3
        {
            let valid_map = match descendant.get(b"CIDToGIDMap") {
                Ok(value) => valid_cid_to_gid_map(self.document, value, self.limits)?,
                Err(_) => false,
            };
            if !valid_map {
                self.invalid_cid_to_gid_maps.push(font_failure(
                    object_id,
                    description,
                    "has a used Type 2 CIDFont descendant without a valid /CIDToGIDMap",
                ));
            }
        }

        let Ok(encoding) = font.get(b"Encoding") else {
            return Ok(());
        };
        // Confirmed live against veraPDF 1.28.2: an *indirect* reference to
        // the name /Identity-H or /Identity-V is accepted exactly like a
        // direct one, so resolution must happen before checking either
        // shape, not only before the stream fallback.
        let resolved = resolve_optional(self.document, encoding, self.limits.max_reference_depth)?;
        if resolved
            .and_then(|object| object.as_name().ok())
            .is_some_and(|name| matches!(name, b"Identity-H" | b"Identity-V"))
        {
            return Ok(());
        }
        let Some(cmap) = resolved.and_then(|object| object.as_stream().ok()) else {
            self.unembedded_cmaps.push(font_failure(
                object_id,
                description,
                "uses a non-Identity CMap that is not embedded",
            ));
            return Ok(());
        };

        let cmap_name = resolved_name(self.document, &cmap.dict, b"CMapName", self.limits)?;
        if cmap_name.is_some_and(|name| matches!(name, b"Identity-H" | b"Identity-V")) {
            return Ok(());
        }

        if let Some(descendant) = descendant
            && cid_system_info(self.document, descendant, self.limits)?
                != cid_system_info(self.document, &cmap.dict, self.limits)?
        {
            self.incompatible_type0_system_info.push(font_failure(
                object_id,
                description,
                "has incompatible CIDSystemInfo Registry or Ordering values in its CIDFont and CMap",
            ));
        }

        let dictionary_wmode = cmap
            .dict
            .get(b"WMode")
            .ok()
            .map(|value| resolved_integer(self.document, self.limits, value))
            .transpose()?
            .flatten()
            .unwrap_or(0);
        let bytes = decode_font_stream(cmap, self.limits)?;
        if cmap_maximal_cid(&bytes).is_some_and(|cid| cid > 65_535) {
            self.invalid_cmap_cids.push(font_failure(
                object_id,
                description,
                "uses an embedded CMap with a CID greater than 65,535",
            ));
        }
        let content_wmode = cmap_content_wmode(&bytes).unwrap_or(0);
        if dictionary_wmode != content_wmode {
            self.invalid_cmap_wmodes.push(font_failure(
                object_id,
                description,
                &format!(
                    "has embedded CMap WMode {content_wmode} but dictionary /WMode {dictionary_wmode}"
                ),
            ));
        }
        Ok(())
    }

    fn inspect_cid_subset_font(
        &mut self,
        font: &Dictionary,
        object_id: Option<PdfObjectId>,
        description: &str,
    ) -> Result<(), PdfError> {
        let is_subset = resolved_name(self.document, font, b"BaseFont", self.limits)?
            .is_some_and(is_subset_font_name);
        if !is_subset {
            return Ok(());
        }
        let has_cid_set = font_descriptor_dictionary(self.document, font, self.limits)?
            .and_then(|descriptor| descriptor.get(b"CIDSet").ok())
            .map(|value| resolve_optional(self.document, value, self.limits.max_reference_depth))
            .transpose()?
            .flatten()
            .is_some_and(|value| value.as_stream().is_ok());
        if !has_cid_set {
            self.invalid_cid_subset_cidsets.push(font_failure(
                object_id,
                description,
                "has a CIDFont subset without a descriptor /CIDSet stream",
            ));
        }
        Ok(())
    }

    fn inspect_font_dictionary(
        &mut self,
        font: &Dictionary,
        object_id: Option<PdfObjectId>,
        description: &str,
        subtype: Option<&str>,
    ) -> Result<(), PdfError> {
        if resolved_name(self.document, font, b"Type", self.limits)? != Some(b"Font".as_slice()) {
            self.invalid_types.push(font_failure(
                object_id,
                description,
                "has a missing or invalid /Type instead of /Font",
            ));
        }

        let base_font = resolved_name(self.document, font, b"BaseFont", self.limits)?;
        if subtype != Some("Type3") && base_font.is_none() {
            self.invalid_base_fonts.push(font_failure(
                object_id,
                description,
                "has a missing or invalid /BaseFont",
            ));
        }

        // Confirmed live against veraPDF 1.28.2: the "standard 14 fonts"
        // /FirstChar//LastChar//Widths exemption applies only to Type1/
        // MMType1 -- a TrueType (or Type3) font whose /BaseFont happens to
        // match a standard-14 name (e.g. "Helvetica") is still required to
        // supply all three, since standard-14 metrics are a Type1 concept.
        let requires_simple_font_metrics = match subtype {
            Some("Type1" | "MMType1") => !base_font.is_some_and(is_standard_14_font),
            Some("TrueType" | "Type3") => true,
            _ => false,
        };
        if requires_simple_font_metrics {
            let first_char = font
                .get(b"FirstChar")
                .ok()
                .map(|value| resolved_integer(self.document, self.limits, value))
                .transpose()?
                .flatten();
            let last_char = font
                .get(b"LastChar")
                .ok()
                .map(|value| resolved_integer(self.document, self.limits, value))
                .transpose()?
                .flatten();
            if first_char.is_none() {
                self.invalid_first_chars.push(font_failure(
                    object_id,
                    description,
                    "has a missing or invalid /FirstChar",
                ));
            }
            if last_char.is_none() {
                self.invalid_last_chars.push(font_failure(
                    object_id,
                    description,
                    "has a missing or invalid /LastChar",
                ));
            }
            let widths_size = resolved_array(self.document, font, b"Widths", self.limits)?
                .and_then(|widths| i64::try_from(widths.len()).ok());
            let expected_size = first_char
                .and_then(|first| last_char.and_then(|last| last.checked_sub(first)))
                .and_then(|difference| difference.checked_add(1));
            if widths_size.is_none() || widths_size != expected_size {
                self.invalid_widths.push(font_failure(
                    object_id,
                    description,
                    "has a missing /Widths array or a size different from /LastChar - /FirstChar + 1",
                ));
            }
        }

        if let Some(invalid_subtype) =
            invalid_embedded_font_subtype(self.document, font, self.limits)?
        {
            self.invalid_font_file_subtypes.push(font_failure(
                object_id,
                description,
                &format!("uses unsupported embedded font subtype /{invalid_subtype}"),
            ));
        }
        Ok(())
    }
}

fn type1_encoding_differences(
    document: &Document,
    font: &Dictionary,
    limits: &SafetyLimits,
) -> Result<BTreeMap<u8, String>, PdfError> {
    let Ok(encoding) = font.get(b"Encoding") else {
        return Ok(BTreeMap::new());
    };
    let Some(encoding) = resolve_optional(document, encoding, limits.max_reference_depth)? else {
        return Ok(BTreeMap::new());
    };
    let Ok(encoding) = encoding.as_dict() else {
        return Ok(BTreeMap::new());
    };
    let Some(differences) = encoding
        .get(b"Differences")
        .ok()
        .map(|value| resolve_optional(document, value, limits.max_reference_depth))
        .transpose()?
        .flatten()
        .and_then(|value| value.as_array().ok())
    else {
        return Ok(BTreeMap::new());
    };
    let mut names = BTreeMap::new();
    let mut code = None;
    for entry in differences {
        if let Some(value) =
            resolved_integer(document, limits, entry)?.and_then(|value| u8::try_from(value).ok())
        {
            code = Some(value);
        } else if let (Some(current), Ok(name)) = (code, entry.as_name()) {
            names.insert(current, String::from_utf8_lossy(name).into_owned());
            code = current.checked_add(1);
        }
    }
    Ok(names)
}

/// A `/CIDToGIDMap` resolved once per font instead of once per rendered
/// CID, since the map itself (`Identity` or a decoded stream) never changes
/// across the many CIDs a single font renders.
enum CidToGidMap {
    Identity,
    Table(Vec<u8>),
    Unavailable,
}

impl CidToGidMap {
    fn glyph_for(&self, cid: u16) -> Option<ttf_parser::GlyphId> {
        match self {
            Self::Identity => Some(ttf_parser::GlyphId(cid)),
            Self::Table(bytes) => {
                let offset = usize::from(cid).checked_mul(2)?;
                let entry = bytes.get(offset..offset.saturating_add(2))?;
                Some(ttf_parser::GlyphId(u16::from_be_bytes([
                    entry[0], entry[1],
                ])))
            }
            Self::Unavailable => None,
        }
    }
}

fn resolve_cid_to_gid_map(
    document: &Document,
    map: &Object,
    limits: &SafetyLimits,
) -> Result<CidToGidMap, PdfError> {
    // Confirmed live against veraPDF 1.28.2 (matching `valid_cid_to_gid_map`):
    // an indirect reference to the name /Identity is accepted exactly like
    // a direct one, so resolution must happen before checking either shape.
    let resolved = resolve_optional(document, map, limits.max_reference_depth)?;
    if resolved.and_then(|value| value.as_name().ok()) == Some(b"Identity".as_slice()) {
        return Ok(CidToGidMap::Identity);
    }
    let Some(stream) = resolved.and_then(|value| value.as_stream().ok()) else {
        return Ok(CidToGidMap::Unavailable);
    };
    Ok(CidToGidMap::Table(decode_font_stream(stream, limits)?))
}

/// A single `/W` array group, in original array order.
enum WGroup {
    /// `c [w1 w2 ... wn]`: width for CID `first + i` is `widths[i]`, when
    /// present and numeric.
    Singles {
        first: u16,
        widths: Vec<Option<f64>>,
    },
    /// `cFirst cLast w`.
    Range { first: u16, last: u16, width: f64 },
}

/// A font's `/W` (and `/DW`) entries, parsed once instead of rescanning the
/// whole array for every rendered CID. `width_for` reproduces
/// the original sequential-scan predicate exactly, including that a
/// malformed group aborts the lookup for every CID from that point on
/// *without* falling back to `/DW` (unlike a fully well-formed array that
/// simply has no matching group).
struct CidWidths {
    groups: Vec<WGroup>,
    truncated: bool,
    default_width: Option<f64>,
}

impl CidWidths {
    fn width_for(&self, cid: u16) -> Option<f64> {
        for group in &self.groups {
            match group {
                WGroup::Singles { first, widths } => {
                    if let Some(offset) = cid.checked_sub(*first)
                        && let Some(width) = widths.get(usize::from(offset))
                    {
                        return *width;
                    }
                }
                WGroup::Range { first, last, width } => {
                    if (*first..=*last).contains(&cid) {
                        return Some(*width);
                    }
                }
            }
        }
        if self.truncated {
            None
        } else {
            self.default_width
        }
    }
}

fn parse_cid_widths(
    document: &Document,
    font: &Dictionary,
    limits: &SafetyLimits,
) -> Result<CidWidths, PdfError> {
    let default_width = font
        .get(b"DW")
        .ok()
        .map(|value| resolved_float(document, limits, value))
        .transpose()?
        .flatten()
        .map(f64::from);
    let none = || CidWidths {
        groups: Vec::new(),
        truncated: false,
        default_width,
    };
    let Ok(value) = font.get(b"W") else {
        return Ok(none());
    };
    let Some(array) = resolve_optional(document, value, limits.max_reference_depth)?
        .and_then(|value| value.as_array().ok())
    else {
        return Ok(none());
    };

    let mut groups = Vec::new();
    let mut index = 0usize;
    let truncated = loop {
        if index >= array.len() {
            break false;
        }
        let Some(first) = array
            .get(index)
            .map(|value| resolved_integer(document, limits, value))
            .transpose()?
            .flatten()
            .and_then(|value| u16::try_from(value).ok())
        else {
            break true;
        };
        let Some(next) = array
            .get(index + 1)
            .map(|value| resolve_optional(document, value, limits.max_reference_depth))
            .transpose()?
            .flatten()
        else {
            break true;
        };
        if let Ok(widths) = next.as_array() {
            // The original scan converts each offset `0..widths.len()` to a
            // `u16` one at a time, aborting the moment that overflows.
            // Reaching offset 65536 is exactly the same overflow point.
            let usable_len = widths.len().min(usize::from(u16::MAX) + 1);
            let overflowed = widths.len() > usable_len;
            let mut resolved_widths = Vec::with_capacity(usable_len);
            for width in &widths[..usable_len] {
                resolved_widths.push(resolved_float(document, limits, width)?.map(f64::from));
            }
            groups.push(WGroup::Singles {
                first,
                widths: resolved_widths,
            });
            if overflowed {
                break true;
            }
            index += 2;
            continue;
        }
        let Some(last) = next
            .as_i64()
            .ok()
            .and_then(|value| u16::try_from(value).ok())
        else {
            break true;
        };
        let Some(width) = array
            .get(index + 2)
            .map(|value| resolved_float(document, limits, value))
            .transpose()?
            .flatten()
        else {
            break true;
        };
        groups.push(WGroup::Range {
            first,
            last,
            width: f64::from(width),
        });
        index += 3;
    };

    Ok(CidWidths {
        groups,
        truncated,
        default_width,
    })
}

fn single_encoded_character(encoding: &lopdf::Encoding<'_>, byte: u8) -> Option<char> {
    let value = encoding.bytes_to_string(&[byte]).ok()?;
    let mut characters = value.chars();
    let character = characters.next()?;
    characters.next().is_none().then_some(character)
}

fn first_descendant_dictionary<'a>(
    document: &'a Document,
    font: &'a Dictionary,
    limits: &SafetyLimits,
) -> Result<Option<&'a Dictionary>, PdfError> {
    let Ok(descendants) = font.get(b"DescendantFonts") else {
        return Ok(None);
    };
    let Some(descendants) = resolve_optional(document, descendants, limits.max_reference_depth)?
        .and_then(|object| object.as_array().ok())
    else {
        return Ok(None);
    };
    let Some(descendant) = descendants.first() else {
        return Ok(None);
    };
    Ok(
        resolve_optional(document, descendant, limits.max_reference_depth)?
            .and_then(|object| object.as_dict().ok()),
    )
}

fn valid_cid_to_gid_map(
    document: &Document,
    value: &Object,
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    // Confirmed live against veraPDF 1.28.2: an *indirect* reference to the
    // name /Identity is accepted exactly like a direct one, so resolution
    // must happen before checking either shape, not only before the stream
    // fallback.
    let Some(resolved) = resolve_optional(document, value, limits.max_reference_depth)? else {
        return Ok(false);
    };
    Ok(resolved.as_name().ok() == Some(b"Identity".as_slice()) || resolved.as_stream().is_ok())
}

fn cid_system_info(
    document: &Document,
    dictionary: &Dictionary,
    limits: &SafetyLimits,
) -> Result<Option<CidSystemInfo>, PdfError> {
    let Ok(info) = dictionary.get(b"CIDSystemInfo") else {
        return Ok(None);
    };
    let Some(info) = resolve_optional(document, info, limits.max_reference_depth)?
        .and_then(|object| object.as_dict().ok())
    else {
        return Ok(None);
    };
    // Confirmed live against veraPDF 1.28.2: an indirect reference to the
    // /Registry or /Ordering string is resolved exactly like a direct one.
    let Some(registry) = resolved_string(document, info, b"Registry", limits)? else {
        return Ok(None);
    };
    let Some(ordering) = resolved_string(document, info, b"Ordering", limits)? else {
        return Ok(None);
    };
    Ok(Some((registry, ordering)))
}

fn resolved_string(
    document: &Document,
    dictionary: &Dictionary,
    key: &[u8],
    limits: &SafetyLimits,
) -> Result<Option<Vec<u8>>, PdfError> {
    let Ok(value) = dictionary.get(key) else {
        return Ok(None);
    };
    Ok(
        resolve_optional(document, value, limits.max_reference_depth)?.and_then(|object| {
            match object {
                Object::String(value, _) => Some(value.clone()),
                _ => None,
            }
        }),
    )
}

/// A dictionary array value (e.g. `/Widths`) resolved through indirection
/// before use, mirroring `resolved_string`/`resolved_integer` -- the array
/// itself is not guaranteed to be direct.
fn resolved_array<'a>(
    document: &'a Document,
    dictionary: &'a Dictionary,
    key: &[u8],
    limits: &SafetyLimits,
) -> Result<Option<&'a Vec<Object>>, PdfError> {
    let Ok(value) = dictionary.get(key) else {
        return Ok(None);
    };
    Ok(
        resolve_optional(document, value, limits.max_reference_depth)?
            .and_then(|object| object.as_array().ok()),
    )
}

/// A dictionary name value (e.g. `/Subtype`) resolved through indirection
/// before use, mirroring `resolved_string`/`resolved_array`.
fn resolved_name<'a>(
    document: &'a Document,
    dictionary: &'a Dictionary,
    key: &[u8],
    limits: &SafetyLimits,
) -> Result<Option<&'a [u8]>, PdfError> {
    let Ok(value) = dictionary.get(key) else {
        return Ok(None);
    };
    Ok(
        resolve_optional(document, value, limits.max_reference_depth)?
            .and_then(|object| object.as_name().ok()),
    )
}

/// A single numeric array element (e.g. a `/Widths` entry) resolved through
/// indirection before use, mirroring `resolved_integer`'s contract for a
/// real-valued rather than integer-valued reference.
fn resolved_float(
    document: &Document,
    limits: &SafetyLimits,
    object: &Object,
) -> Result<Option<f32>, PdfError> {
    Ok(
        resolve_optional(document, object, limits.max_reference_depth)?
            .and_then(|object| object.as_float().ok()),
    )
}

fn decode_font_stream(stream: &Stream, limits: &SafetyLimits) -> Result<Vec<u8>, PdfError> {
    match stream.decompressed_content_with_limit(limits.max_decoded_stream_size) {
        Ok(bytes) => Ok(bytes),
        Err(lopdf::Error::Decompress(lopdf::DecompressError::MemoryLimitExceeded { .. })) => {
            Err(PdfError::FontDecodeLimit(limits.max_decoded_stream_size))
        }
        Err(_) => Ok(stream.content.clone()),
    }
}

fn cmap_content_wmode(bytes: &[u8]) -> Option<i64> {
    let mut tokens = bytes
        .split(|byte| byte.is_ascii_whitespace())
        .filter(|token| !token.is_empty());
    while let Some(token) = tokens.next() {
        if token == b"/WMode" {
            return std::str::from_utf8(tokens.next()?).ok()?.parse().ok();
        }
    }
    None
}

/// Clamps a CMap's self-declared entry count to the number of entries the
/// remaining token buffer could actually hold, so a tiny malicious stream
/// declaring an astronomical count (e.g. `99999999999999 begincidrange
/// <00> <01> 1 endcidrange`, ~50 bytes) cannot force a near-unbounded loop:
/// every claimed entry beyond the real buffer size would only ever find
/// missing tokens via `.get()` anyway, so clamping changes no outcome for a
/// well-formed or merely under-provisioned declaration.
fn bounded_cmap_entry_count(
    declared: usize,
    tokens: &[&[u8]],
    entries_start: usize,
    entry_width: usize,
) -> usize {
    declared.min(tokens.len().saturating_sub(entries_start) / entry_width)
}

fn cmap_maximal_cid(bytes: &[u8]) -> Option<u32> {
    let tokens = bytes
        .split(|byte| byte.is_ascii_whitespace())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let mut maximum = None;
    let mut cursor = 0usize;
    while cursor < tokens.len() {
        let Some(count) = tokens[cursor]
            .iter()
            .all(u8::is_ascii_digit)
            .then(|| {
                std::str::from_utf8(tokens[cursor])
                    .ok()?
                    .parse::<usize>()
                    .ok()
            })
            .flatten()
        else {
            cursor += 1;
            continue;
        };
        match tokens.get(cursor + 1).copied() {
            Some(b"begincidchar") => {
                let count = bounded_cmap_entry_count(count, &tokens, cursor + 2, 2);
                for entry in 0..count {
                    if let Some(cid) = tokens
                        .get(cursor + 3 + entry * 2)
                        .and_then(|token| parse_cmap_integer(token))
                    {
                        maximum = Some(maximum.unwrap_or(cid).max(cid));
                    }
                }
                cursor += 2 + count * 2;
            }
            Some(b"begincidrange") => {
                let count = bounded_cmap_entry_count(count, &tokens, cursor + 2, 3);
                for entry in 0..count {
                    let base = cursor + 2 + entry * 3;
                    let Some(start) = tokens.get(base).and_then(|token| parse_cmap_hex(token))
                    else {
                        continue;
                    };
                    let Some(end) = tokens.get(base + 1).and_then(|token| parse_cmap_hex(token))
                    else {
                        continue;
                    };
                    let Some(cid) = tokens
                        .get(base + 2)
                        .and_then(|token| parse_cmap_integer(token))
                    else {
                        continue;
                    };
                    maximum = Some(
                        maximum
                            .unwrap_or(cid)
                            .max(cid.saturating_add(end.saturating_sub(start))),
                    );
                }
                cursor += 2 + count * 3;
            }
            _ => cursor += 1,
        }
    }
    maximum
}

/// A Type0 font's resolved `/Encoding`, cacheable per indirect CMap object
/// since it never changes across the many rendered-byte sequences checked
/// against it.
enum CmapDecoder {
    /// The literal name `/Identity-H`/`/Identity-V`, or an embedded CMap
    /// that itself `usecmap`s one of them: every two shown bytes decode
    /// directly as a big-endian CID.
    IdentityBytes,
    /// A parsed explicit embedded CMap.
    Parsed(ParsedCmap),
    /// `encoding` did not resolve to a usable CMap.
    Unavailable,
}

impl CmapDecoder {
    fn decode(&self, shown_bytes: &[u8]) -> Option<Vec<u16>> {
        match self {
            Self::IdentityBytes => identity_cids(shown_bytes),
            Self::Parsed(parsed) => parsed.decode(shown_bytes),
            Self::Unavailable => None,
        }
    }
}

fn identity_cids(shown_bytes: &[u8]) -> Option<Vec<u16>> {
    let mut chunks = shown_bytes.chunks_exact(2);
    let cids = chunks
        .by_ref()
        .map(|cid| u16::from_be_bytes([cid[0], cid[1]]))
        .collect();
    chunks.remainder().is_empty().then_some(cids)
}

fn resolve_cmap_decoder(
    document: &Document,
    encoding: &Object,
    limits: &SafetyLimits,
) -> Result<CmapDecoder, PdfError> {
    let resolved = resolve_optional(document, encoding, limits.max_reference_depth)?;
    // Confirmed live against veraPDF 1.28.2: an indirect reference to the
    // name /Identity-H or /Identity-V is accepted exactly like a direct one.
    if resolved
        .and_then(|object| object.as_name().ok())
        .is_some_and(|name| matches!(name, b"Identity-H" | b"Identity-V"))
    {
        return Ok(CmapDecoder::IdentityBytes);
    }
    let Some(cmap) = resolved.and_then(|object| object.as_stream().ok()) else {
        return Ok(CmapDecoder::Unavailable);
    };
    let bytes = decode_font_stream(cmap, limits)?;
    if cmap_uses_identity_base(&bytes) {
        return Ok(CmapDecoder::IdentityBytes);
    }
    Ok(match parse_cmap(&bytes) {
        Some(parsed) => CmapDecoder::Parsed(parsed),
        None => CmapDecoder::Unavailable,
    })
}

#[derive(Clone, Copy)]
struct CmapCodeSpace {
    bytes: usize,
    start: u32,
    end: u32,
}

#[derive(Clone, Copy)]
struct CmapCidRange {
    bytes: usize,
    start: u32,
    end: u32,
    first_cid: u16,
}

fn cmap_uses_identity_base(bytes: &[u8]) -> bool {
    let tokens = cmap_tokens(bytes);
    tokens
        .windows(2)
        .any(|pair| matches!(pair[0], b"/Identity-H" | b"/Identity-V") && pair[1] == b"usecmap")
}

/// An embedded CMap's code spaces, single-CID mappings, and CID ranges,
/// parsed once from its raw bytes and reused for every rendered-byte
/// sequence decoded through it.
struct ParsedCmap {
    code_spaces: Vec<CmapCodeSpace>,
    chars: BTreeMap<(usize, u32), u16>,
    ranges: Vec<CmapCidRange>,
}

/// Resolves explicit CID CMaps with one- through four-byte code spaces. Other
/// inherited maps remain inapplicable until the predefined CMap collection is
/// modeled.
fn parse_cmap(bytes: &[u8]) -> Option<ParsedCmap> {
    let tokens = cmap_tokens(bytes);
    let mut cursor = 0usize;
    let mut code_spaces = Vec::new();
    let mut chars = BTreeMap::new();
    let mut ranges = Vec::new();
    while cursor + 1 < tokens.len() {
        let Some(count) = tokens[cursor]
            .iter()
            .all(u8::is_ascii_digit)
            .then(|| {
                std::str::from_utf8(tokens[cursor])
                    .ok()?
                    .parse::<usize>()
                    .ok()
            })
            .flatten()
        else {
            cursor += 1;
            continue;
        };
        match tokens[cursor + 1] {
            b"begincodespacerange" => {
                let count = bounded_cmap_entry_count(count, &tokens, cursor + 2, 2);
                for entry in 0..count {
                    let base = cursor + 2 + entry * 2;
                    let (Some(start), Some(end)) = (
                        tokens.get(base).and_then(|token| parse_cmap_code(token)),
                        tokens
                            .get(base + 1)
                            .and_then(|token| parse_cmap_code(token)),
                    ) else {
                        continue;
                    };
                    if start.0 == end.0 && start.1 <= end.1 {
                        code_spaces.push(CmapCodeSpace {
                            bytes: start.0,
                            start: start.1,
                            end: end.1,
                        });
                    }
                }
                cursor += 2 + count * 2;
            }
            b"begincidchar" => {
                let count = bounded_cmap_entry_count(count, &tokens, cursor + 2, 2);
                for entry in 0..count {
                    let base = cursor + 2 + entry * 2;
                    let (Some(code), Some(cid)) = (
                        tokens.get(base).and_then(|token| parse_cmap_code(token)),
                        tokens
                            .get(base + 1)
                            .and_then(|token| parse_cmap_integer(token)),
                    ) else {
                        continue;
                    };
                    if cid <= 65_535 {
                        chars.insert(code, cid as u16);
                    }
                }
                cursor += 2 + count * 2;
            }
            b"begincidrange" => {
                let count = bounded_cmap_entry_count(count, &tokens, cursor + 2, 3);
                for entry in 0..count {
                    let base = cursor + 2 + entry * 3;
                    let (Some(start), Some(end), Some(cid)) = (
                        tokens.get(base).and_then(|token| parse_cmap_code(token)),
                        tokens
                            .get(base + 1)
                            .and_then(|token| parse_cmap_code(token)),
                        tokens
                            .get(base + 2)
                            .and_then(|token| parse_cmap_integer(token)),
                    ) else {
                        continue;
                    };
                    if start.0 == end.0
                        && start.1 <= end.1
                        && cid.saturating_add(end.1 - start.1) <= 65_535
                    {
                        ranges.push(CmapCidRange {
                            bytes: start.0,
                            start: start.1,
                            end: end.1,
                            first_cid: cid as u16,
                        });
                    }
                }
                cursor += 2 + count * 3;
            }
            _ => cursor += 1,
        }
    }
    if code_spaces.is_empty() {
        return None;
    }
    code_spaces.sort_by_key(|space| space.bytes);
    Some(ParsedCmap {
        code_spaces,
        chars,
        ranges,
    })
}

impl ParsedCmap {
    fn decode(&self, shown_bytes: &[u8]) -> Option<Vec<u16>> {
        let mut cids = Vec::new();
        let mut position = 0usize;
        while position < shown_bytes.len() {
            let mut decoded = None;
            for space in &self.code_spaces {
                let Some(end) = position.checked_add(space.bytes) else {
                    continue;
                };
                let Some(code_bytes) = shown_bytes.get(position..end) else {
                    continue;
                };
                let code = code_bytes
                    .iter()
                    .fold(0_u32, |value, byte| value << 8 | u32::from(*byte));
                if code < space.start || code > space.end {
                    continue;
                }
                let cid = self.chars.get(&(space.bytes, code)).copied().or_else(|| {
                    self.ranges
                        .iter()
                        .find(|range| {
                            range.bytes == space.bytes && range.start <= code && code <= range.end
                        })
                        .map(|range| range.first_cid + (code - range.start) as u16)
                });
                decoded = Some((space.bytes, cid));
                break;
            }
            let Some((width, Some(cid))) = decoded else {
                return None;
            };
            position += width;
            cids.push(cid);
        }
        Some(cids)
    }
}

fn cmap_tokens(bytes: &[u8]) -> Vec<&[u8]> {
    bytes
        .split(|byte| matches!(*byte, b'\r' | b'\n'))
        .flat_map(|line| line.split(|byte| *byte == b'%').next())
        .flat_map(|line| line.split(|byte| byte.is_ascii_whitespace()))
        .filter(|token| !token.is_empty())
        .collect()
}

fn parse_cmap_integer(token: &[u8]) -> Option<u32> {
    std::str::from_utf8(token).ok()?.parse().ok()
}

fn parse_cmap_code(token: &[u8]) -> Option<(usize, u32)> {
    let token = token.strip_prefix(b"<")?.strip_suffix(b">")?;
    if token.is_empty() || token.len() % 2 != 0 || token.len() > 8 {
        return None;
    }
    let value = std::str::from_utf8(token)
        .ok()
        .and_then(|token| u32::from_str_radix(token, 16).ok())?;
    Some((token.len() / 2, value))
}

fn parse_cmap_hex(token: &[u8]) -> Option<u32> {
    parse_cmap_code(token).map(|(_, value)| value)
}

fn is_subset_font_name(name: &[u8]) -> bool {
    name.len() >= 7 && name[..6].iter().all(u8::is_ascii_uppercase) && name[6] == b'+'
}

fn cid_set_contains(bytes: &[u8], cid: u16) -> bool {
    bytes
        .get(usize::from(cid) / 8)
        .is_some_and(|byte| byte & (1 << (7 - cid % 8)) != 0)
}

fn type1_charset_names(bytes: &[u8]) -> BTreeSet<&str> {
    std::str::from_utf8(bytes)
        .ok()
        .into_iter()
        .flat_map(|value| value.split('/').skip(1))
        .filter_map(|value| value.split_ascii_whitespace().next())
        .collect()
}

/// Decrypts Adobe Type 1 eexec/charstring ciphertext (Type 1 Font Format
/// spec §7): `initial_state` is 55665 for the outer eexec layer and 4330 for
/// individual charstrings; `skip` discards the corresponding lenIV-controlled
/// random leading bytes (conventionally 4 for eexec, `lenIV` for charstrings).
fn type1_decrypt(bytes: &[u8], initial_state: u16, skip: usize) -> Vec<u8> {
    let mut state = initial_state;
    bytes
        .iter()
        .copied()
        .map(|ciphertext| {
            let plaintext = ciphertext ^ (state >> 8) as u8;
            state = state
                .wrapping_add(u16::from(ciphertext))
                .wrapping_mul(52_845)
                .wrapping_add(22_719);
            plaintext
        })
        .skip(skip)
        .collect()
}

fn type1_program_char_names(bytes: &[u8]) -> BTreeSet<String> {
    let bytes = type1_pfb_payload(bytes);
    let Some(eexec) = bytes.windows(5).position(|window| window == b"eexec") else {
        return BTreeSet::new();
    };
    let ciphertext = type1_eexec_ciphertext(&bytes[eexec + 5..]);
    let plaintext = type1_decrypt(&ciphertext, 55_665, 4);
    let Some(start) = plaintext
        .windows(b"/CharStrings".len())
        .position(|window| window == b"/CharStrings")
    else {
        return BTreeSet::new();
    };
    plaintext[start..]
        .split(|byte| *byte == b'/')
        .skip(1)
        .filter_map(|entry| {
            let name = entry
                .iter()
                .take_while(|byte| byte.is_ascii_alphanumeric() || **byte == b'.')
                .copied()
                .collect::<Vec<_>>();
            (!name.is_empty()).then(|| String::from_utf8_lossy(&name).into_owned())
        })
        .collect()
}

fn type1_program_charstring_widths(bytes: &[u8]) -> BTreeMap<String, f64> {
    let bytes = type1_pfb_payload(bytes);
    let Some(eexec) = bytes.windows(5).position(|window| window == b"eexec") else {
        return BTreeMap::new();
    };
    let ciphertext = type1_eexec_ciphertext(&bytes[eexec + 5..]);
    let plaintext = type1_decrypt(&ciphertext, 55_665, 4);
    let len_iv = plaintext
        .windows(b"/lenIV".len())
        .position(|window| window == b"/lenIV")
        .and_then(|position| parse_ascii_integer(&plaintext[position + 6..]))
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(4);
    let Some(charstrings) = plaintext
        .windows(b"/CharStrings".len())
        .position(|window| window == b"/CharStrings")
    else {
        return BTreeMap::new();
    };
    let mut result = BTreeMap::new();
    let mut position = charstrings + b"/CharStrings".len();
    while let Some(relative) = plaintext[position..].iter().position(|byte| *byte == b'/') {
        position += relative + 1;
        let name_start = position;
        while plaintext
            .get(position)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'.')
        {
            position += 1;
        }
        if position == name_start {
            continue;
        }
        let name = String::from_utf8_lossy(&plaintext[name_start..position]).into_owned();
        while plaintext
            .get(position)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            position += 1;
        }
        let Some(length) = parse_ascii_integer(&plaintext[position..])
            .and_then(|value| usize::try_from(value).ok())
        else {
            break;
        };
        while plaintext
            .get(position)
            .is_some_and(|byte| !byte.is_ascii_whitespace())
        {
            position += 1;
        }
        while plaintext
            .get(position)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            position += 1;
        }
        if plaintext.get(position..position + 2) != Some(b"RD") {
            continue;
        }
        position += 2;
        while plaintext
            .get(position)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            position += 1;
        }
        let Some(end) = position.checked_add(length) else {
            break;
        };
        let Some(charstring) = plaintext.get(position..end) else {
            break;
        };
        if let Some(width) = type1_charstring_width(charstring, len_iv) {
            result.insert(name, width);
        }
        position = end;
    }
    result
}

fn type1_charstring_width(bytes: &[u8], len_iv: usize) -> Option<f64> {
    let decrypted = type1_decrypt(bytes, 4_330, len_iv);
    let mut operands = Vec::new();
    let mut position = 0;
    while position < decrypted.len() {
        if let Some((value, consumed)) = cff_number(&decrypted[position..]) {
            operands.push(value);
            position += consumed;
            continue;
        }
        let byte = decrypted[position];
        if byte == 13 && operands.len() >= 2 {
            return operands.get(1).copied();
        }
        if byte == 12 && decrypted.get(position + 1) == Some(&7) && operands.len() >= 3 {
            return operands.get(2).copied();
        }
        return None;
    }
    None
}

fn parse_ascii_integer(bytes: &[u8]) -> Option<i64> {
    let mut end = 0;
    while bytes
        .get(end)
        .is_some_and(|byte| byte.is_ascii_whitespace() || byte.is_ascii_digit() || *byte == b'-')
    {
        end += 1;
    }
    std::str::from_utf8(&bytes[..end]).ok()?.trim().parse().ok()
}

fn type1_eexec_ciphertext(bytes: &[u8]) -> Vec<u8> {
    let bytes = bytes
        .iter()
        .skip_while(|byte| byte.is_ascii_whitespace())
        .copied()
        .collect::<Vec<_>>();
    let hex = bytes
        .iter()
        .copied()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    if hex.len() % 2 == 0 && !hex.is_empty() && hex.iter().all(u8::is_ascii_hexdigit) {
        return hex
            .chunks_exact(2)
            .map(|pair| {
                let digit = |byte| match byte {
                    b'0'..=b'9' => byte - b'0',
                    b'a'..=b'f' => byte - b'a' + 10,
                    b'A'..=b'F' => byte - b'A' + 10,
                    _ => unreachable!("checked ASCII hexadecimal byte"),
                };
                digit(pair[0]) << 4 | digit(pair[1])
            })
            .collect();
    }
    bytes
}

fn cff_cid_glyph_width(bytes: &[u8], glyph_id: u16) -> Option<f64> {
    let (_, top_offset) = cff_index(bytes, 4)?;
    let (top_items, _) = cff_index(bytes, top_offset)?;
    let top = top_items.first().copied()?;
    let top_dict = cff_dict(top);
    let charstrings_offset = cff_dict_offset(&top_dict, 17)?;
    let (charstrings, _) = cff_index(bytes, charstrings_offset)?;
    let charstring = charstrings.get(usize::from(glyph_id))?;
    let fd_array_offset = cff_dict_offset(&top_dict, 1236)?;
    let fd_select_offset = cff_dict_offset(&top_dict, 1237)?;
    let fd_index = cff_fd_select(bytes, fd_select_offset, glyph_id)?;
    let (fd_array, _) = cff_index(bytes, fd_array_offset)?;
    let fd_dict = cff_dict(fd_array.get(usize::from(fd_index))?);
    let private_values = fd_dict.get(&18)?;
    let private_size = usize::try_from(private_values.first().copied()? as i64).ok()?;
    let private_offset = usize::try_from(private_values.get(1).copied()? as i64).ok()?;
    let private_end = private_offset.checked_add(private_size)?;
    let private = cff_dict(bytes.get(private_offset..private_end)?);
    let default_width = private
        .get(&20)
        .and_then(|values| values.first().copied())
        .unwrap_or(0.0);
    let nominal_width = private
        .get(&21)
        .and_then(|values| values.first().copied())
        .unwrap_or(0.0);
    let width = cff_charstring_width(charstring, nominal_width, default_width)?;
    let matrix = top_dict
        .get(&1207)
        .and_then(|values| values.first().copied())
        .unwrap_or(0.001);
    Some(width * matrix * 1000.0)
}

fn cff_charstring_width(bytes: &[u8], nominal: f64, default: f64) -> Option<f64> {
    let mut operands = Vec::new();
    let mut position = 0usize;
    while position < bytes.len() {
        let byte = *bytes.get(position)?;
        if let Some((value, consumed)) = cff_number(bytes.get(position..)?) {
            operands.push(value);
            position = position.checked_add(consumed)?;
            continue;
        }
        let operator = if byte == 12 {
            let second = *bytes.get(position)?;
            1200 + u16::from(second)
        } else {
            u16::from(byte)
        };
        if operator == 14 {
            return Some(if operands.len() == 1 {
                nominal + operands[0]
            } else {
                default
            });
        }
        return Some(if operands.len() % 2 == 1 {
            nominal + operands[0]
        } else {
            default
        });
    }
    None
}

fn cff_fd_select(bytes: &[u8], offset: usize, glyph_id: u16) -> Option<u8> {
    let format = *bytes.get(offset)?;
    match format {
        0 => bytes
            .get(offset.checked_add(1 + usize::from(glyph_id))?)
            .copied(),
        3 => {
            let count = u16::from_be_bytes([*bytes.get(offset + 1)?, *bytes.get(offset + 2)?]);
            let mut position = offset + 3;
            let mut previous = 0_u16;
            for _ in 0..count {
                let first = u16::from_be_bytes([*bytes.get(position)?, *bytes.get(position + 1)?]);
                let fd = *bytes.get(position + 2)?;
                if previous <= glyph_id && glyph_id < first {
                    return Some(fd);
                }
                previous = first;
                position += 3;
            }
            None
        }
        _ => None,
    }
}

fn cff_dict_offset(dict: &BTreeMap<u16, Vec<f64>>, operator: u16) -> Option<usize> {
    usize::try_from(dict.get(&operator)?.first().copied()? as i64).ok()
}

fn cff_dict(bytes: &[u8]) -> BTreeMap<u16, Vec<f64>> {
    let mut result = BTreeMap::new();
    let mut operands = Vec::new();
    let mut position = 0usize;
    while position < bytes.len() {
        if let Some((value, consumed)) = cff_number(&bytes[position..]) {
            operands.push(value);
            position += consumed;
            continue;
        }
        let byte = bytes[position];
        position += 1;
        let operator = if byte == 12 {
            let Some(second) = bytes.get(position).copied() else {
                break;
            };
            position += 1;
            1200 + u16::from(second)
        } else {
            u16::from(byte)
        };
        result.insert(operator, std::mem::take(&mut operands));
    }
    result
}

fn cff_number(bytes: &[u8]) -> Option<(f64, usize)> {
    let first = *bytes.first()?;
    match first {
        32..=246 => Some((f64::from(first) - 139.0, 1)),
        247..=250 => Some((
            f64::from(first - 247) * 256.0 + f64::from(*bytes.get(1)?) + 108.0,
            2,
        )),
        251..=254 => Some((
            -(f64::from(first - 251) * 256.0 + f64::from(*bytes.get(1)?) + 108.0),
            2,
        )),
        28 => Some((
            f64::from(i16::from_be_bytes([*bytes.get(1)?, *bytes.get(2)?])),
            3,
        )),
        29 => Some((
            f64::from(i32::from_be_bytes([
                *bytes.get(1)?,
                *bytes.get(2)?,
                *bytes.get(3)?,
                *bytes.get(4)?,
            ])),
            5,
        )),
        255 => Some((
            f64::from(i32::from_be_bytes([
                *bytes.get(1)?,
                *bytes.get(2)?,
                *bytes.get(3)?,
                *bytes.get(4)?,
            ])) / 65_536.0,
            5,
        )),
        _ => None,
    }
}

fn cff_index(bytes: &[u8], offset: usize) -> Option<(Vec<&[u8]>, usize)> {
    let count = usize::from(u16::from_be_bytes([
        *bytes.get(offset)?,
        *bytes.get(offset + 1)?,
    ]));
    if count == 0 {
        return Some((Vec::new(), offset + 2));
    }
    let offsize = usize::from(*bytes.get(offset + 2)?);
    if !(1..=4).contains(&offsize) {
        return None;
    }
    let offsets_start = offset + 3;
    let data_start = offsets_start.checked_add((count + 1).checked_mul(offsize)?)?;
    let read_offset = |index: usize| -> Option<usize> {
        let start = offsets_start.checked_add(index.checked_mul(offsize)?)?;
        let mut value = 0usize;
        for byte in bytes.get(start..start + offsize)? {
            value = value.checked_mul(256)?.checked_add(usize::from(*byte))?;
        }
        (value > 0).then_some(value - 1)
    };
    let mut items = Vec::with_capacity(count);
    for index in 0..count {
        let start = data_start.checked_add(read_offset(index)?)?;
        let end = data_start.checked_add(read_offset(index + 1)?)?;
        items.push(bytes.get(start..end)?);
    }
    let end = data_start.checked_add(read_offset(count)?)?;
    Some((items, end))
}

fn type1_pfb_payload(bytes: &[u8]) -> Vec<u8> {
    if !bytes.starts_with(&[0x80, 0x01]) {
        return bytes.to_vec();
    }
    let mut payload = Vec::new();
    let mut position = 0;
    while bytes.get(position) == Some(&0x80) {
        let Some(kind) = bytes.get(position + 1) else {
            break;
        };
        if *kind == 3 {
            break;
        }
        let Some(length) = bytes
            .get(position + 2..position + 6)
            .and_then(|length| length.try_into().ok())
            .map(u32::from_le_bytes)
            .and_then(|length| usize::try_from(length).ok())
        else {
            break;
        };
        let start = position + 6;
        let Some(end) = start.checked_add(length) else {
            break;
        };
        let Some(segment) = bytes.get(start..end) else {
            break;
        };
        payload.extend_from_slice(segment);
        position = end;
    }
    payload
}

fn type1_standard_glyph_name(byte: u8) -> Option<&'static str> {
    const NAMES: [&str; 95] = [
        "space",
        "exclam",
        "quotedbl",
        "numbersign",
        "dollar",
        "percent",
        "ampersand",
        "quoteright",
        "parenleft",
        "parenright",
        "asterisk",
        "plus",
        "comma",
        "hyphen",
        "period",
        "slash",
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "colon",
        "semicolon",
        "less",
        "equal",
        "greater",
        "question",
        "at",
        "A",
        "B",
        "C",
        "D",
        "E",
        "F",
        "G",
        "H",
        "I",
        "J",
        "K",
        "L",
        "M",
        "N",
        "O",
        "P",
        "Q",
        "R",
        "S",
        "T",
        "U",
        "V",
        "W",
        "X",
        "Y",
        "Z",
        "bracketleft",
        "backslash",
        "bracketright",
        "asciicircum",
        "underscore",
        "quoteleft",
        "a",
        "b",
        "c",
        "d",
        "e",
        "f",
        "g",
        "h",
        "i",
        "j",
        "k",
        "l",
        "m",
        "n",
        "o",
        "p",
        "q",
        "r",
        "s",
        "t",
        "u",
        "v",
        "w",
        "x",
        "y",
        "z",
        "braceleft",
        "bar",
        "braceright",
        "asciitilde",
    ];
    NAMES.get(usize::from(byte.checked_sub(b' ')?)).copied()
}

fn font_descriptor_dictionary<'a>(
    document: &'a Document,
    font: &'a Dictionary,
    limits: &SafetyLimits,
) -> Result<Option<&'a Dictionary>, PdfError> {
    let Ok(descriptor) = font.get(b"FontDescriptor") else {
        return Ok(None);
    };
    Ok(
        resolve_optional(document, descriptor, limits.max_reference_depth)?
            .and_then(|object| object.as_dict().ok()),
    )
}

fn truetype_encoding(
    document: &Document,
    font: &Dictionary,
    limits: &SafetyLimits,
) -> Result<(Option<Vec<u8>>, bool), PdfError> {
    let Ok(encoding) = font.get(b"Encoding") else {
        return Ok((None, false));
    };
    let Some(encoding) = resolve_optional(document, encoding, limits.max_reference_depth)? else {
        return Ok((None, false));
    };
    if let Ok(name) = encoding.as_name() {
        return Ok((Some(name.to_vec()), false));
    }
    let Ok(dictionary) = encoding.as_dict() else {
        return Ok((None, false));
    };
    let base = resolved_name(document, dictionary, b"BaseEncoding", limits)?.map(ToOwned::to_owned);
    Ok((base, contains_key(dictionary, b"Differences")))
}

fn truetype_cmap_count(
    document: &Document,
    descriptor: Option<&Dictionary>,
    limits: &SafetyLimits,
) -> Result<Option<usize>, PdfError> {
    let Some(descriptor) = descriptor else {
        return Ok(None);
    };
    let Ok(file) = descriptor.get(b"FontFile2") else {
        return Ok(None);
    };
    let Some(stream) = resolve_optional(document, file, limits.max_reference_depth)?
        .and_then(|object| object.as_stream().ok())
    else {
        return Ok(None);
    };
    let bytes = decode_font_stream(stream, limits)?;
    // Confirmed live against veraPDF 1.28.2: it reads the `cmap` table's
    // subtable count directly from the SFNT table directory, independent
    // of whether the rest of the font (`maxp`, `hhea`, ...) otherwise
    // parses -- a font whose `cmap` table is valid but whose `maxp` table
    // is malformed still gets this predicate evaluated. `RawFace` reads
    // only the table directory, unlike `Face::parse`, which additionally
    // requires several unrelated mandatory tables to succeed.
    Ok(ttf_parser::RawFace::parse(&bytes, 0)
        .ok()
        .and_then(|face| face.table(ttf_parser::Tag::from_bytes(b"cmap")))
        .and_then(ttf_parser::cmap::Table::parse)
        .map(|cmap| usize::from(cmap.subtables.len())))
}

fn font_is_embedded(
    document: &Document,
    font: &Dictionary,
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    let Some(descriptor) = font_descriptor_dictionary(document, font, limits)? else {
        return Ok(false);
    };
    // `containsFontFile` is a veraPDF model property, not mere key presence:
    // the pinned malformed-program case proves that the stream must be
    // recognized as an embedded font program.
    for key in [b"FontFile".as_slice(), b"FontFile2", b"FontFile3"] {
        if let Ok(file) = descriptor.get(key)
            && let Some(stream) = resolve_optional(document, file, limits.max_reference_depth)?
                .and_then(|object| object.as_stream().ok())
            && valid_font_program(document, key, stream, limits)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn invalid_embedded_font_subtype(
    document: &Document,
    font: &Dictionary,
    limits: &SafetyLimits,
) -> Result<Option<String>, PdfError> {
    let Some(descriptor) = font_descriptor_dictionary(document, font, limits)? else {
        return Ok(None);
    };
    for key in [b"FontFile".as_slice(), b"FontFile2", b"FontFile3"] {
        let Ok(file) = descriptor.get(key) else {
            continue;
        };
        let Some(stream) = resolve_optional(document, file, limits.max_reference_depth)?
            .and_then(|object| object.as_stream().ok())
        else {
            continue;
        };
        let Some(subtype) = resolved_name(document, &stream.dict, b"Subtype", limits)? else {
            continue;
        };
        if !matches!(subtype, b"Type1C" | b"CIDFontType0C") {
            return Ok(Some(String::from_utf8_lossy(subtype).into_owned()));
        }
    }
    Ok(None)
}

fn valid_font_program(
    document: &Document,
    key: &[u8],
    stream: &Stream,
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    let bytes = match stream.decompressed_content_with_limit(limits.max_decoded_stream_size) {
        Ok(bytes) => bytes,
        Err(lopdf::Error::Decompress(lopdf::DecompressError::MemoryLimitExceeded { .. })) => {
            return Err(PdfError::FontDecodeLimit(limits.max_decoded_stream_size));
        }
        Err(_) => return Ok(false),
    };
    Ok(match key {
        // Confirmed live against veraPDF 1.28.2: a stream whose bytes start
        // with the Type1 magic header but never contain an `eexec` marker
        // (no encrypted/binary section at all, just header text) still
        // fails `PDFA1B-FONT-EMBEDDING-001` on veraPDF -- the header alone
        // is not sufficient, matching the PFB/Type1 format's own
        // requirement that a real program always has an encrypted segment.
        b"FontFile" => {
            (bytes.starts_with(b"%!PS-AdobeFont")
                && bytes.windows(5).any(|window| window == b"eexec"))
                || (bytes.starts_with(&[0x80, 0x01])
                    && bytes.windows(2).any(|window| window == [0x80, 0x02]))
        }
        b"FontFile2" => valid_sfnt(&bytes),
        // Confirmed live against veraPDF 1.28.2: a stream whose bytes
        // satisfy only the CFF header-byte shape (major/minor version,
        // hdrSize) but contain no parseable CFF structure beyond that still
        // fails `PDFA1B-FONT-EMBEDDING-001` -- the header alone is not
        // sufficient. Uses the same `ttf_parser::cff::Table::parse` already
        // relied on for glyph lookups (`inspect_rendered_cff_type1_glyphs`/
        // `inspect_rendered_cff_cidfont_glyphs`), for consistency.
        b"FontFile3" => {
            matches!(
                resolved_name(document, &stream.dict, b"Subtype", limits)?,
                Some(b"Type1C" | b"CIDFontType0C")
            ) && ttf_parser::cff::Table::parse(&bytes).is_some()
        }
        _ => false,
    })
}

// Confirmed live against veraPDF 1.28.2 (the same fixture that confirmed
// `truetype_cmap_count`'s fix): it still considers a `/FontFile2` stream
// "embedded" (no `PDFA1B-FONT-EMBEDDING-001` failure) even when the font's
// `maxp` table is malformed enough that a full `ttf_parser::Face::parse`
// fails, as long as the SFNT signature and table directory are themselves
// readable. `RawFace::parse` reads only the table directory, matching that
// narrower bar, instead of requiring every mandatory table
// (`head`/`hhea`/`maxp`/`hmtx`/glyph outlines) to individually succeed.
fn valid_sfnt(bytes: &[u8]) -> bool {
    ttf_parser::RawFace::parse(bytes, 0).is_ok()
}

fn object_key(object: &Object, context: &str, name: Option<&Object>) -> ResourceKey {
    match object {
        Object::Reference(id) => ResourceKey::Indirect(*id),
        _ => ResourceKey::Direct(format!(
            "{context}/{}",
            name.and_then(|value| value.as_name().ok())
                .map(|value| String::from_utf8_lossy(value).into_owned())
                .unwrap_or_else(|| "direct".to_owned())
        )),
    }
}

fn describe_font(object: &Object, context: &str, name: Option<&Object>) -> String {
    match object {
        Object::Reference((number, generation)) => {
            format!("font object {number} {generation}")
        }
        _ => format!(
            "direct font {} in {context}",
            name.and_then(|value| value.as_name().ok())
                .map(|value| format!("/{}", String::from_utf8_lossy(value)))
                .unwrap_or_else(|| "(unnamed)".to_owned())
        ),
    }
}

fn describe_descendant(object: &Object, parent: &str) -> String {
    match object {
        Object::Reference((number, generation)) => {
            format!("descendant font object {number} {generation}")
        }
        _ => format!("direct descendant font of {parent}"),
    }
}

fn shows_text(operands: &[Object]) -> bool {
    operands.iter().any(|operand| match operand {
        Object::String(bytes, _) => !bytes.is_empty(),
        Object::Array(items) => shows_text(items),
        _ => false,
    })
}

fn shown_text_bytes(operands: &[Object]) -> Vec<u8> {
    let mut bytes = Vec::new();
    collect_shown_text_bytes(operands, &mut bytes);
    bytes
}

fn collect_shown_text_bytes(operands: &[Object], bytes: &mut Vec<u8>) {
    for operand in operands {
        match operand {
            Object::String(value, _) => bytes.extend_from_slice(value),
            Object::Array(items) => collect_shown_text_bytes(items, bytes),
            _ => {}
        }
    }
}

/// Resolves `object` through indirection before checking whether it is an
/// integer. Confirmed live against veraPDF 1.28.2 that an *indirect*
/// numeric value (e.g. a CMap's `/WMode`) is resolved rather than treated
/// as absent -- the several integer-valued keys this file reads (`/WMode`,
/// `/Flags`, `/FirstChar`, `/LastChar`) are not guaranteed to be direct.
fn resolved_integer(
    document: &Document,
    limits: &SafetyLimits,
    object: &Object,
) -> Result<Option<i64>, PdfError> {
    Ok(
        resolve_optional(document, object, limits.max_reference_depth)?
            .and_then(|object| object.as_i64().ok()),
    )
}

fn is_standard_14_font(name: &[u8]) -> bool {
    matches!(
        name,
        b"Courier"
            | b"Courier-Bold"
            | b"Courier-Oblique"
            | b"Courier-BoldOblique"
            | b"Helvetica"
            | b"Helvetica-Bold"
            | b"Helvetica-Oblique"
            | b"Helvetica-BoldOblique"
            | b"Times-Roman"
            | b"Times-Bold"
            | b"Times-Italic"
            | b"Times-BoldItalic"
            | b"Symbol"
            | b"ZapfDingbats"
    )
}

fn font_failure(object_id: Option<PdfObjectId>, description: &str, detail: &str) -> RuleFailure {
    RuleFailure {
        object_id,
        description: format!("{description} {detail}"),
    }
}

#[cfg(test)]
mod tests {
    use lopdf::{Dictionary, Document, Object, Stream};

    use super::{
        cff_fd_select, cff_index, cmap_maximal_cid, cmap_uses_identity_base,
        inspect_all_embedded_cmap_cids, parse_cmap, shown_text_bytes, type1_eexec_ciphertext,
        type1_pfb_payload, type1_program_char_names, type1_program_charstring_widths,
    };
    use crate::SafetyLimits;

    /// A malicious CMap can declare an astronomical entry count while
    /// supplying almost no real tokens (`99999999999999 begincidrange <00>
    /// <01> 1 endcidrange` is ~50 bytes). Before `bounded_cmap_entry_count`
    /// existed, both `cmap_maximal_cid` and `parse_cmap` trusted the
    /// declared count directly as a loop bound, so this alone hung the
    /// validator (confirmed: still running after 30+ seconds). The fix
    /// clamps the count to what the token buffer could actually hold, so
    /// this must now return almost immediately.
    #[test]
    fn huge_declared_cmap_entry_count_does_not_hang() {
        let malicious = b"99999999999999 begincidrange <00> <01> 1 endcidrange";
        let started = std::time::Instant::now();
        assert_eq!(cmap_maximal_cid(malicious), Some(2));
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "cmap_maximal_cid took {:?} on a malicious declared count",
            started.elapsed()
        );

        let malicious_parse =
            b"1 begincodespacerange <00> <FF> endcodespacerange 99999999999999 begincidrange <00> <01> 1 endcidrange";
        let started = std::time::Instant::now();
        let parsed = parse_cmap(malicious_parse).expect("parse malicious cmap");
        assert_eq!(parsed.decode(&[0x00]), Some(vec![1]));
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "parse_cmap took {:?} on a malicious declared count",
            started.elapsed()
        );
    }

    #[test]
    fn finds_maximum_cid_in_char_and_range_mappings() {
        assert_eq!(
            cmap_maximal_cid(b"2 begincidchar <00> 10 <01> 65536 endcidchar"),
            Some(65_536)
        );
        assert_eq!(
            cmap_maximal_cid(b"1 begincidrange <00> <FF> 65500 endcidrange"),
            Some(65_755)
        );
        assert_eq!(
            cmap_maximal_cid(b"1 begincidrange <00> <FF> 0 endcidrange"),
            Some(255)
        );
    }

    #[test]
    fn decodes_explicit_variable_width_cid_cmaps() {
        let cmap = b"% 1 begincidchar <41> 99 endcidchar\n2 begincodespacerange\n<00> <7F>\n<8000> <80FF>\nendcodespacerange\n1 begincidchar\n<41> 12\nendcidchar\n1 begincidrange\n<8000> <8002> 20\nendcidrange";
        let cmap_cids =
            |shown_bytes: &[u8]| parse_cmap(cmap).and_then(|parsed| parsed.decode(shown_bytes));
        assert_eq!(cmap_cids(&[0x41, 0x80, 0x01]), Some(vec![12, 21]));
        assert_eq!(cmap_cids(&[0x80]), None);
        assert_eq!(cmap_cids(&[0x42]), None);
    }

    #[test]
    fn recognizes_identity_usecmap_bases() {
        assert!(cmap_uses_identity_base(b"/Identity-H usecmap"));
        assert!(cmap_uses_identity_base(b"/Identity-V\nusecmap"));
        assert!(!cmap_uses_identity_base(b"/NotIdentity usecmap"));
    }

    #[test]
    fn extracts_names_from_an_encrypted_type1_private_dictionary() {
        let plaintext = [
            vec![0; 4],
            b"dup /Private 1 dict dup begin /CharStrings 1 dict dup begin /space 1 RD".to_vec(),
        ]
        .concat();
        let mut state = 55_665_u16;
        let encrypted: Vec<_> = plaintext
            .into_iter()
            .map(|plaintext| {
                let ciphertext = plaintext ^ (state >> 8) as u8;
                state = state
                    .wrapping_add(u16::from(ciphertext))
                    .wrapping_mul(52_845)
                    .wrapping_add(22_719);
                ciphertext
            })
            .collect();
        let bytes = [b"%!PS-AdobeFont\neexec\n".as_slice(), encrypted.as_slice()].concat();
        assert!(super::type1_program_char_names(&bytes).contains("space"));

        let hex = encrypted
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<String>();
        let hex_bytes = [b"%!PS-AdobeFont\neexec\n".as_slice(), hex.as_bytes()].concat();
        assert!(super::type1_program_char_names(&hex_bytes).contains("space"));
    }

    #[test]
    fn maps_printable_type1_encoding_bytes_to_adobe_glyph_names() {
        assert_eq!(super::type1_standard_glyph_name(b' '), Some("space"));
        assert_eq!(super::type1_standard_glyph_name(b'A'), Some("A"));
        assert_eq!(super::type1_standard_glyph_name(b'z'), Some("z"));
        assert_eq!(super::type1_standard_glyph_name(0x80), None);
    }

    #[test]
    fn finds_oversized_cids_in_unused_embedded_cmaps() {
        let mut document = Document::with_version("1.4");
        let mut dictionary = Dictionary::new();
        dictionary.set("CMapName", "Unused-CMap");
        document.add_object(Stream::new(
            dictionary,
            b"1 begincidchar <00> 65536 endcidchar".to_vec(),
        ));
        let failures = inspect_all_embedded_cmap_cids(&document, &SafetyLimits::default())
            .expect("inspect CMaps");
        assert_eq!(failures.len(), 1);
    }

    /// Every hand-rolled font byte-parser in this file must not panic on
    /// truncated or empty input, since embedded font programs are attacker
    /// controlled. Reaching the final assertions at all (rather than
    /// aborting on a panic) is the test; the specific `None`/empty return
    /// values are incidental. This does not exercise `ttf_parser`/`lopdf`
    /// -backed paths (`RawFace`, `cff::Table`), which are already
    /// panic-free by the external crate's own contract.
    #[test]
    fn hand_rolled_font_parsers_do_not_panic_on_truncated_input() {
        for truncated in [b"".as_slice(), b"\x00", b"\x00\x01\x02", b"\x80\x01\x00"] {
            let _ = cff_fd_select(truncated, 0, 0);
            let _ = cff_index(truncated, 0);
            let _ = parse_cmap(truncated);
            assert!(type1_program_char_names(truncated).is_empty());
            assert!(type1_program_charstring_widths(truncated).is_empty());
            let _ = type1_eexec_ciphertext(truncated);
            let _ = type1_pfb_payload(truncated);
        }
        // A `cff_fd_select` format-3 range table declaring a huge count
        // with almost no backing bytes must not hang (same class of bug as
        // `bounded_cmap_entry_count` guards for CMaps): each iteration's
        // `bytes.get(...)` bounds check fails immediately once the real
        // data is exhausted, since `count` is read as a fixed 2-byte `u16`
        // (capped at 65,535) rather than parsed from unbounded decimal
        // text.
        let huge_count_fd_select = [3u8, 0xFF, 0xFF, 0, 1, 0];
        let started = std::time::Instant::now();
        assert_eq!(cff_fd_select(&huge_count_fd_select, 0, 5), None);
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }

    #[test]
    fn retains_only_string_bytes_from_text_show_operands() {
        assert_eq!(
            shown_text_bytes(&[
                Object::String(b"A".to_vec(), lopdf::StringFormat::Literal),
                Object::Array(vec![
                    Object::String(b"B".to_vec(), lopdf::StringFormat::Literal),
                    Object::Integer(-120),
                    Object::String(b"C".to_vec(), lopdf::StringFormat::Literal),
                ]),
            ]),
            b"ABC"
        );
    }
}
