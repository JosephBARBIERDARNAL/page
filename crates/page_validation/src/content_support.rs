//! Executes PDF content streams to discover the resources and content properties used by
//! font, colour-space, graphics, XObject, and accessibility checks.
//!
//! The executor follows page content, Form XObjects, and annotation appearances while tracking
//! graphics state, text runs, and marked content. Shared resource lookups and decoded-stream
//! caching avoid repeated work, with limits on decoded bytes, nesting, and Form invocations. Marked content is tagged only when its ParentTree connection reaches the structure root. Exhaustive reporting also records the page using each resource so shared objects can be located across pages.
//! Page Contents arrays are decoded as one logical byte sequence, retaining stream boundaries for diagnostic attribution.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::object_resolution::{
    ResourceKey, contains_key, resolve_optional, resolved_integer, resolved_name, walk_inherited,
};
use crate::page_tree::PageEntry;
use crate::report::RuleFailure;
use crate::syntax::{is_pdf_boundary, is_pdf_whitespace};

// lopdf commits to parsing an image dictionary after BI, so a stripped image
// needs a different operator to keep ordinary operations decodable.
const INLINE_IMAGE_OPERATOR: &str = "pageInlineImage";

#[derive(Clone, Debug)]
pub(crate) struct SelectedColorSpace {
    pub(crate) value: Object,
    pub(crate) context: String,
}

#[derive(Clone, Debug)]
pub(crate) struct XObjectUse {
    pub(crate) key: ResourceKey,
    pub(crate) object: Object,
    pub(crate) painted: bool,
    pub(crate) alternate: bool,
    pub(crate) explicit_mask: bool,
    pub(crate) soft_mask: bool,
    pub(crate) appearance: bool,
    pub(crate) occurrences: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct FormXObjectUse {
    pub(crate) contains_mcid: bool,
    pub(crate) references: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum XObjectUseKind {
    Painted,
    Appearance,
    Alternate,
    ExplicitMask,
    SoftMask,
}

impl XObjectUse {
    fn record(&mut self, kind: XObjectUseKind) {
        self.occurrences += 1;
        match kind {
            XObjectUseKind::Painted => self.painted = true,
            XObjectUseKind::Appearance => self.appearance = true,
            XObjectUseKind::Alternate => self.alternate = true,
            XObjectUseKind::ExplicitMask => self.explicit_mask = true,
            XObjectUseKind::SoftMask => self.soft_mask = true,
        }
    }

    pub(crate) fn is_ordinary_image(&self) -> bool {
        self.painted || self.alternate || self.soft_mask
    }

    pub(crate) fn has_declared_xobject_role(&self) -> bool {
        self.is_ordinary_image() || self.explicit_mask
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ExtGStateUse {
    pub(crate) key: ResourceKey,
    pub(crate) dictionary: Dictionary,
}

#[derive(Clone, Debug)]
pub(crate) struct FontUse {
    pub(crate) key: ResourceKey,
    pub(crate) object: Object,
    pub(crate) description: String,
    pub(crate) rendering_mode: i64,
    pub(crate) shown_bytes: Vec<u8>,
    pub(crate) text_runs: Vec<FontTextRun>,
}

#[derive(Clone, Debug)]
pub(crate) struct FontTextRun {
    pub(crate) shown_bytes: Vec<u8>,
    pub(crate) actual_text_present: bool,
    pub(crate) page_object_id: Option<ObjectId>,
    pub(crate) marked_content_id: Option<i64>,
}

/// Deterministic discoveries made while executing the bounded page, Form,
/// annotation-appearance, tiling-Pattern, and rendered-Type3 content graph.
/// Consumers apply their own rule predicates to this one shared population
/// instead of maintaining subtly different reachability walkers.
#[derive(Clone, Debug, Default)]
pub(crate) struct ContentExecutionSummary {
    pub(crate) page_objects: BTreeMap<u32, BTreeSet<ObjectId>>,
    /// Decoded content bytes already charged to the document-wide stream budget.
    pub(crate) decoded_content_bytes: usize,
    pub(crate) selected_color_spaces: Vec<SelectedColorSpace>,
    pub(crate) xobjects: BTreeMap<ResourceKey, XObjectUse>,
    pub(crate) has_annotations: bool,
    pub(crate) has_widget_annotations: bool,
    pub(crate) has_page_or_annotation_actions: bool,
    // Keyed by indirect object identity so aliases in different resource
    // dictionaries still share the PDF/UA-1 7.20-2 reference count.
    pub(crate) form_xobjects: BTreeMap<ObjectId, FormXObjectUse>,
    pub(crate) extgstates: Vec<ExtGStateUse>,
    pub(crate) fonts: Vec<FontUse>,
    pub(crate) excessive_graphics_state_nesting: Vec<RuleFailure>,
    pub(crate) invalid_rendering_intents: BTreeMap<String, String>,
    pub(crate) undefined_operators: BTreeMap<String, String>,
    pub(crate) inline_image_lzw_context: Option<String>,
    pub(crate) inline_image_invalid_filter_context: Option<String>,
    pub(crate) inline_image_interpolate_context: Option<String>,
    pub(crate) has_odd_hex_string: bool,
    pub(crate) has_non_hex_character: bool,
    pub(crate) out_of_range_integers: Vec<RuleFailure>,
    pub(crate) out_of_range_reals: Vec<RuleFailure>,
    pub(crate) underflow_reals_pdfa_2: Vec<RuleFailure>,
    pub(crate) overlong_names: Vec<RuleFailure>,
    pub(crate) overlong_strings: Vec<RuleFailure>,
    pub(crate) overlong_strings_pdfa_2: Vec<RuleFailure>,
    pub(crate) language_failures: Vec<RuleFailure>,
    pub(crate) language_failures_pdfa23: Vec<RuleFailure>,
    pub(crate) language_failures_pdfua1: Vec<RuleFailure>,
    pub(crate) text_language_failures: Vec<RuleFailure>,
    pub(crate) span_actual_text_language_failures: Vec<RuleFailure>,
    pub(crate) span_alt_text_language_failures: Vec<RuleFailure>,
    pub(crate) span_expansion_text_language_failures: Vec<RuleFailure>,
    pub(crate) artifacts_inside_tagged_content: Vec<RuleFailure>,
    pub(crate) tagged_content_inside_artifacts: Vec<RuleFailure>,
    pub(crate) untagged_content: Vec<RuleFailure>,
    pub(crate) uses_default_gray: bool,
    pub(crate) inherited_resources: Vec<RuleFailure>,
    pub(crate) icc_cmyk_overprint: Vec<RuleFailure>,
    pub(crate) pages_with_transparency: BTreeSet<u32>,
}

pub(crate) fn decode_content_stream(
    stream: &Stream,
    limits: &SafetyLimits,
) -> Result<Vec<u8>, PdfError> {
    let bytes = match stream.decompressed_content_with_limit(limits.max_decoded_stream_size) {
        Ok(bytes) => bytes,
        Err(lopdf::Error::Decompress(lopdf::DecompressError::MemoryLimitExceeded { .. })) => {
            return Err(PdfError::ContentDecodeLimit(limits.max_decoded_stream_size));
        }
        Err(_) if stream.content.len() <= limits.max_decoded_stream_size => stream.content.clone(),
        Err(_) => return Err(PdfError::ContentDecodeLimit(limits.max_decoded_stream_size)),
    };
    if bytes.len() > limits.max_decoded_stream_size {
        return Err(PdfError::ContentDecodeLimit(limits.max_decoded_stream_size));
    }
    Ok(bytes)
}

/// A content-stream decode cache keyed by the stream's own indirect object id.
/// Repeated invocations in the shared executor reuse decoded bytes while each
/// invocation still applies its own graphics state and resource context.
pub(crate) type ContentCache = HashMap<ObjectId, Rc<[u8]>>;

/// Decodes `stream`'s content like [`decode_content_stream`], reusing a
/// previous decode of the same indirect object from `cache` instead of
/// decompressing it again. `object_id` is the reference that resolved to
/// `stream` (a page `/Contents` entry or an invoked Form XObject), if any;
/// a directly embedded stream has no id to key on and is decoded but not
/// cached, exactly as it always was.
///
/// The cache is bounded by the document-wide decoded-content budget: only a
/// fresh decode consumes it, while a cache hit reuses bytes already counted.
pub(crate) fn decode_content_stream_cached(
    stream: &Stream,
    object_id: Option<ObjectId>,
    cache: &mut ContentCache,
    limits: &SafetyLimits,
    total_decoded_bytes: &mut usize,
) -> Result<Rc<[u8]>, PdfError> {
    if let Some(object_id) = object_id
        && let Some(cached) = cache.get(&object_id)
    {
        return Ok(Rc::clone(cached));
    }
    let bytes: Rc<[u8]> = Rc::from(decode_content_stream(stream, limits)?);
    let total =
        total_decoded_bytes
            .checked_add(bytes.len())
            .ok_or(PdfError::TotalContentDecodeLimit(
                limits.max_total_decoded_content_size,
            ))?;
    if total > limits.max_total_decoded_content_size {
        return Err(PdfError::TotalContentDecodeLimit(
            limits.max_total_decoded_content_size,
        ));
    }
    *total_decoded_bytes = total;
    if let Some(object_id) = object_id {
        cache.insert(object_id, Rc::clone(&bytes));
    }
    Ok(bytes)
}

pub(crate) fn inherited_page_resources<'a>(
    document: &'a Document,
    node: &'a Dictionary,
    limits: &SafetyLimits,
) -> Result<Option<&'a Dictionary>, PdfError> {
    walk_inherited(
        document,
        node,
        limits,
        b"Resources",
        |document, value, limits| {
            Ok(
                resolve_optional(document, value, limits.max_reference_depth)?
                    .and_then(|object| object.as_dict().ok()),
            )
        },
    )
}

/// Visits every annotation reached through a page `/Annots` array, resolving
/// the array and each entry while deduplicating by indirect object id via
/// the caller-owned `inspected` set. `visit` receives the resolved
/// (unconverted) annotation object; callers extract a dictionary from it
/// however is appropriate for their check (some accept only a plain
/// dictionary, others also accept a stream-backed one via `dictionary_based`).
pub(crate) fn for_each_page_annotation<'a>(
    document: &'a Document,
    pages: &'a [PageEntry],
    limits: &SafetyLimits,
    inspected: &mut BTreeSet<ObjectId>,
    mut visit: impl FnMut(
        u32,
        usize,
        Option<PdfObjectId>,
        &'a Dictionary,
        &'a Object,
    ) -> Result<(), PdfError>,
) -> Result<(), PdfError> {
    for (index, page_entry) in pages.iter().enumerate() {
        let page_number = (index + 1) as u32;
        let Some(page) = page_entry.resolve(document) else {
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
            let Some(resolved) =
                resolve_optional(document, annotation, limits.max_reference_depth)?
            else {
                continue;
            };
            visit(
                page_number,
                index,
                object_id.map(Into::into),
                page,
                resolved,
            )?;
        }
    }
    Ok(())
}

pub(crate) fn resource_once<'a>(
    document: &'a Document,
    limits: &SafetyLimits,
    resources: Option<&'a Dictionary>,
    category: &[u8],
    name: &[u8],
) -> Result<Option<&'a Object>, PdfError> {
    let Some(resources) = resources else {
        return Ok(None);
    };
    let Ok(category) = resources.get(category) else {
        return Ok(None);
    };
    let Some(category) = resolve_optional(document, category, limits.max_reference_depth)? else {
        return Ok(None);
    };
    let Ok(category) = category.as_dict() else {
        return Ok(None);
    };
    Ok(category.get(name).ok())
}

pub(crate) fn execute_content(
    document: &Document,
    pages: &[PageEntry],
    cache: &mut ContentCache,
    tagged_text_language: &BTreeSet<(ObjectId, i64)>,
    limits: &SafetyLimits,
    collect_locations: bool,
) -> Result<ContentExecutionSummary, PdfError> {
    let mut executor = ContentExecutor {
        document,
        limits,
        cache,
        tagged_text_language,
        tagging: crate::content_tagging::ContentTagging::new(document, limits)?,
        summary: ContentExecutionSummary::default(),
        font_indices: BTreeMap::new(),
        form_stack: Vec::new(),
        form_invocations: 0,
        current_page: 0,
        current_page_object_id: None,
        collect_locations,
    };
    let mut total_decoded_bytes = 0usize;
    for (index, page_entry) in pages.iter().enumerate() {
        let page_number = (index + 1) as u32;
        let page = page_entry
            .resolve(document)
            .ok_or(PdfError::UnexpectedObject("page is not a dictionary"))?;
        executor.execute_page(
            page_number,
            page_entry.object_id(),
            page,
            &mut total_decoded_bytes,
        )?;
    }
    executor.summary.decoded_content_bytes = total_decoded_bytes;
    Ok(executor.summary)
}

pub(crate) fn check_content_limits(
    document: &Document,
    pages: &[PageEntry],
    limits: &SafetyLimits,
) -> Result<(), PdfError> {
    let mut cache = ContentCache::new();
    let mut total_decoded_bytes = 0usize;
    for page_entry in pages {
        let page = page_entry
            .resolve(document)
            .ok_or(PdfError::UnexpectedObject("page is not a dictionary"))?;
        let Ok(contents) = page.get(b"Contents") else {
            continue;
        };
        collect_content_streams(
            document,
            contents,
            &mut cache,
            limits,
            &mut total_decoded_bytes,
            &mut Vec::new(),
            0,
        )?;
    }
    Ok(())
}

struct ContentPart {
    end: usize,
    object_id: Option<ObjectId>,
    bytes: Rc<[u8]>,
}

fn content_object_at(parts: &[ContentPart], offset: usize) -> Option<ObjectId> {
    parts
        .get(parts.partition_point(|part| part.end <= offset))
        .and_then(|part| part.object_id)
}

fn collect_content_streams(
    document: &Document,
    object: &Object,
    cache: &mut ContentCache,
    limits: &SafetyLimits,
    total_decoded_bytes: &mut usize,
    parts: &mut Vec<ContentPart>,
    depth: usize,
) -> Result<(), PdfError> {
    if depth > limits.max_reference_depth {
        return Err(PdfError::ReferenceDepth(limits.max_reference_depth));
    }
    let object_id = object.as_reference().ok();
    let Some(resolved) = resolve_optional(document, object, limits.max_reference_depth)? else {
        return Ok(());
    };
    match resolved {
        Object::Array(items) => {
            for item in items {
                collect_content_streams(
                    document,
                    item,
                    cache,
                    limits,
                    total_decoded_bytes,
                    parts,
                    depth + 1,
                )?;
            }
        }
        Object::Stream(stream) => {
            let bytes = decode_content_stream_cached(
                stream,
                object_id,
                cache,
                limits,
                total_decoded_bytes,
            )?;
            // Cached streams count once towards decoding, but repeated references
            // still occupy space in the logical Contents sequence.
            let end = parts
                .last()
                .map_or(0, |part| part.end)
                .checked_add(usize::from(!parts.is_empty() && !bytes.is_empty()))
                .and_then(|end| end.checked_add(bytes.len()))
                .filter(|end| *end <= limits.max_total_decoded_content_size)
                .ok_or(PdfError::TotalContentDecodeLimit(
                    limits.max_total_decoded_content_size,
                ))?;
            if !bytes.is_empty() {
                parts.push(ContentPart {
                    end,
                    object_id,
                    bytes,
                });
            }
        }
        _ => {}
    }
    Ok(())
}

struct ContentExecutor<'a> {
    collect_locations: bool,
    document: &'a Document,
    limits: &'a SafetyLimits,
    cache: &'a mut ContentCache,
    tagged_text_language: &'a BTreeSet<(ObjectId, i64)>,
    tagging: crate::content_tagging::ContentTagging<'a>,
    summary: ContentExecutionSummary,
    font_indices: BTreeMap<ResourceKey, usize>,
    form_stack: Vec<ObjectId>,
    form_invocations: usize,
    current_page: u32,
    current_page_object_id: Option<ObjectId>,
}

#[derive(Clone, Copy)]
struct ResourceContext<'a> {
    struct_parents: Option<i64>,
    resources: Option<&'a Dictionary>,
    page_resources: Option<&'a Dictionary>,
    resources_are_inherited: bool,
    inspect_pdfua_content: bool,
    uncolored_type3_glyph: bool,
}

#[derive(Clone, Debug, Default)]
struct GraphicsState {
    stroking_pattern: bool,
    nonstroking_pattern: bool,
    font: Option<SelectedFont>,
    rendering_mode: i64,
    stroking_overprint: bool,
    nonstroking_overprint: bool,
    overprint_mode: i64,
    stroking_icc_cmyk: bool,
    nonstroking_icc_cmyk: bool,
    stroking_color_space_selected: bool,
    nonstroking_color_space_selected: bool,
}

#[derive(Clone, Copy)]
struct ColorSpaceSelection {
    is_pattern: bool,
    is_icc_cmyk: bool,
}

#[derive(Clone, Debug)]
struct SelectedFont {
    key: ResourceKey,
    object: Object,
    description: String,
}

#[derive(Clone, Copy)]
struct MarkedContent {
    actual_text_present: bool,
    lang_present: bool,
    mcid: Option<i64>,
    is_artifact: bool,
    is_tagged_content: Option<bool>,
}

impl ContentExecutor<'_> {
    fn record_page_use(&mut self, object: &Object) {
        if self.collect_locations {
            crate::page_locations::references(
                object,
                self.summary
                    .page_objects
                    .entry(self.current_page)
                    .or_default(),
                self.limits.max_reference_depth,
            );
        }
    }

    fn execute_page(
        &mut self,
        page_number: u32,
        page_object_id: Option<ObjectId>,
        page: &Dictionary,
        decoded_bytes: &mut usize,
    ) -> Result<(), PdfError> {
        self.current_page = page_number;
        self.current_page_object_id = page_object_id;
        let resources = inherited_page_resources(self.document, page, self.limits)?.cloned();
        let resources_are_inherited = page.get(b"Resources").is_err();
        self.summary.has_page_or_annotation_actions |= contains_key(page, b"AA");
        let mut active_forms = BTreeSet::new();
        let mut graphics_state = GraphicsState::default();
        let mut graphics_stack = Vec::new();
        let mut marked_content = Vec::new();
        self.record_group_color_space(
            page,
            resources.as_ref(),
            resources.as_ref(),
            &format!("page {page_number}/Group"),
        )?;
        if let Ok(contents) = page.get(b"Contents") {
            self.execute_contents(
                contents,
                ResourceContext {
                    struct_parents: resolved_integer(
                        self.document,
                        page,
                        b"StructParents",
                        self.limits.max_reference_depth,
                    )?,
                    resources: resources.as_ref(),
                    page_resources: resources.as_ref(),
                    resources_are_inherited,
                    inspect_pdfua_content: true,
                    uncolored_type3_glyph: false,
                },
                &mut graphics_state,
                &mut graphics_stack,
                &mut marked_content,
                &mut active_forms,
                decoded_bytes,
                &format!("page {page_number}"),
                0,
            )?;
        }
        self.execute_annotation_appearances(
            page_number,
            page,
            resources.as_ref(),
            &mut active_forms,
            decoded_bytes,
        )?;
        Ok(())
    }

    fn execute_annotation_appearances(
        &mut self,
        page_number: u32,
        page: &Dictionary,
        page_resources: Option<&Dictionary>,
        active_forms: &mut BTreeSet<ObjectId>,
        decoded_bytes: &mut usize,
    ) -> Result<(), PdfError> {
        let Ok(annotations) = page.get(b"Annots") else {
            return Ok(());
        };
        let Some(annotations) =
            resolve_optional(self.document, annotations, self.limits.max_reference_depth)?
                .and_then(|object| object.as_array().ok())
        else {
            return Ok(());
        };
        self.summary.has_annotations |= !annotations.is_empty();
        for (annotation_index, annotation) in annotations.iter().enumerate() {
            let Some(annotation) =
                resolve_optional(self.document, annotation, self.limits.max_reference_depth)?
                    .and_then(|object| object.as_dict().ok())
            else {
                continue;
            };
            self.summary.has_widget_annotations |= resolved_name(
                self.document,
                annotation,
                b"Subtype",
                self.limits.max_reference_depth,
            )? == Some(b"Widget".as_slice());
            self.summary.has_page_or_annotation_actions |=
                contains_key(annotation, b"A") || contains_key(annotation, b"AA");
            let Ok(appearances) = annotation.get(b"AP") else {
                continue;
            };
            let Some(appearances) =
                resolve_optional(self.document, appearances, self.limits.max_reference_depth)?
                    .and_then(|object| object.as_dict().ok())
            else {
                continue;
            };
            for key in [b"N".as_slice(), b"R".as_slice(), b"D".as_slice()] {
                let Ok(entry) = appearances.get(key) else {
                    continue;
                };
                self.execute_appearance_entry(
                    entry,
                    page_resources,
                    active_forms,
                    decoded_bytes,
                    &format!(
                        "page {page_number}/annotation {annotation_index}/appearance /{}",
                        String::from_utf8_lossy(key)
                    ),
                    0,
                )?;
            }
        }
        Ok(())
    }

    fn execute_appearance_entry(
        &mut self,
        entry: &Object,
        page_resources: Option<&Dictionary>,
        active_forms: &mut BTreeSet<ObjectId>,
        decoded_bytes: &mut usize,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        if depth > self.limits.max_reference_depth {
            return Err(PdfError::ReferenceDepth(self.limits.max_reference_depth));
        }
        let Some(resolved) =
            resolve_optional(self.document, entry, self.limits.max_reference_depth)?
        else {
            return Ok(());
        };
        if resolved.as_stream().is_ok() {
            let graphics_state = GraphicsState::default();
            return self.execute_xobject(
                entry,
                XObjectUseKind::Appearance,
                ResourceContext {
                    struct_parents: None,
                    resources: page_resources,
                    page_resources,
                    resources_are_inherited: false,
                    inspect_pdfua_content: false,
                    uncolored_type3_glyph: false,
                },
                &graphics_state,
                &mut Vec::new(),
                active_forms,
                decoded_bytes,
                context,
                depth + 1,
            );
        }
        if depth > 0 {
            return Ok(());
        }
        let Ok(states) = resolved.as_dict() else {
            return Ok(());
        };
        for (name, state) in states.iter() {
            self.execute_appearance_entry(
                state,
                page_resources,
                active_forms,
                decoded_bytes,
                &format!("{context}/{}", String::from_utf8_lossy(name)),
                depth + 1,
            )?;
        }
        Ok(())
    }

    fn execute_contents(
        &mut self,
        contents: &Object,
        resource_context: ResourceContext<'_>,
        graphics_state: &mut GraphicsState,
        graphics_stack: &mut Vec<GraphicsState>,
        marked_content: &mut Vec<MarkedContent>,
        active_forms: &mut BTreeSet<ObjectId>,
        decoded_bytes: &mut usize,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        let ResourceContext {
            struct_parents,
            resources,
            page_resources,
            resources_are_inherited,
            inspect_pdfua_content,
            uncolored_type3_glyph,
        } = resource_context;
        if depth > self.limits.max_reference_depth {
            return Err(PdfError::ReferenceDepth(self.limits.max_reference_depth));
        }
        let mut parts = Vec::new();
        collect_content_streams(
            self.document,
            contents,
            self.cache,
            self.limits,
            decoded_bytes,
            &mut parts,
            depth,
        )?;
        let Some(first) = parts.first() else {
            return Ok(());
        };
        let bytes = if parts.len() == 1 {
            Cow::Borrowed(first.bytes.as_ref())
        } else {
            let mut bytes = Vec::with_capacity(parts.last().map_or(0, |part| part.end));
            for part in &parts {
                // Contents members end at lexical-token boundaries. Separate
                // them so adjacent operators/numbers and trailing comments do
                // not consume the next member's first token (ISO 32000 Table 30;
                // whitespace between members is explicit in ISO 32000-2 Table 31).
                if !bytes.is_empty() {
                    bytes.push(b'\n');
                }
                bytes.extend_from_slice(&part.bytes);
            }
            Cow::Owned(bytes)
        };
        let mut inherited_resources = BTreeMap::<_, BTreeSet<_>>::new();
        let inline_images = tokenize_inline_images(&bytes);
        let ordinary_content = inline_images.ordinary_content.as_ref();
        let object_at = |offset| content_object_at(&parts, inline_images.original_offset(offset));
        let mut inline_image_markers = inline_images.markers.iter();
        for (inline, offset) in inline_images
            .images
            .iter()
            .zip(&inline_images.image_offsets)
        {
            if !uncolored_type3_glyph && let Some(name) = &inline.color_space {
                let _ = self.record_color_space(
                    Object::Name(name.clone()),
                    resources,
                    page_resources,
                    resources_are_inherited,
                    format!("{context}/inline image"),
                    inherited_resources
                        .entry(content_object_at(&parts, *offset))
                        .or_default(),
                )?;
            }
            if inline.has_lzw {
                self.summary
                    .inline_image_lzw_context
                    .get_or_insert_with(|| format!("{context}/inline image"));
            }
            if inline.has_invalid_pdfa2_filter {
                self.summary
                    .inline_image_invalid_filter_context
                    .get_or_insert_with(|| format!("{context}/inline image"));
            }
            if inline.has_interpolate {
                self.summary
                    .inline_image_interpolate_context
                    .get_or_insert_with(|| format!("{context}/inline image"));
            }
            if let Some(name) = &inline.rendering_intent {
                let name = String::from_utf8_lossy(name).into_owned();
                if !is_standard_rendering_intent(&name) {
                    self.summary
                        .invalid_rendering_intents
                        .entry(name)
                        .or_insert_with(|| format!("{context}/inline image"));
                }
            }
        }

        if !content_syntax_is_balanced(ordinary_content) {
            return Ok(());
        }
        inspect_content_syntax_limits(ordinary_content, &object_at, context, &mut self.summary);
        let normalized_content = normalize_digit_suffixed_operators(ordinary_content);
        let Ok(content) = Content::decode(normalized_content.as_ref()) else {
            return Ok(());
        };
        let mut operator_cursor = 0;
        for operation in content.operations {
            // Attribute an operation to its operator's stream even when its
            // operands began in a preceding member of the Contents array.
            let content_id = if parts.len() == 1 {
                first.object_id
            } else {
                next_operator_offset(ordinary_content, &mut operator_cursor, &operation.operator)
                    .and_then(&object_at)
            };
            let inherited_resource_names = inherited_resources.entry(content_id).or_default();
            for operand in &operation.operands {
                collect_content_object_limit_findings(
                    operand,
                    content_id.map(Into::into),
                    &format!("{context}/{}", operation.operator),
                    &mut self.summary,
                );
            }
            let is_inline_image = operation.operator == INLINE_IMAGE_OPERATOR
                && inline_image_markers.next().copied().unwrap_or(false);
            if !is_inline_image && !is_pdf_1_4_operator(&operation.operator) {
                self.summary
                    .undefined_operators
                    .entry(operation.operator.clone())
                    .or_insert_with(|| context.to_owned());
            }
            match operation.operator.as_str() {
                "BMC" => {
                    self.record_artifact_if_nested(
                        operation
                            .operands
                            .first()
                            .and_then(|operand| operand.as_name().ok())
                            == Some(b"Artifact".as_slice()),
                        content_id,
                        marked_content,
                        context,
                    );
                    marked_content.push(MarkedContent {
                        actual_text_present: false,
                        lang_present: false,
                        mcid: None,
                        is_artifact: operation
                            .operands
                            .first()
                            .and_then(|operand| operand.as_name().ok())
                            == Some(b"Artifact".as_slice()),
                        is_tagged_content: None,
                    });
                }
                "BDC" | "DP" => {
                    let properties = operation.operands.last();
                    let Some(properties) = properties else {
                        continue;
                    };
                    let properties = if let Ok(name) = properties.as_name() {
                        resource(
                            self.document,
                            self.limits,
                            resources,
                            page_resources,
                            resources_are_inherited,
                            b"Properties",
                            name,
                            Some(&mut *inherited_resource_names),
                        )?
                    } else {
                        Some(properties)
                    };
                    let properties = properties
                        .map(|properties| {
                            resolve_optional(
                                self.document,
                                properties,
                                self.limits.max_reference_depth,
                            )
                        })
                        .transpose()?
                        .flatten()
                        .and_then(|object| object.as_dict().ok());
                    self.record_artifact_if_nested(
                        operation
                            .operands
                            .first()
                            .and_then(|operand| operand.as_name().ok())
                            == Some(b"Artifact".as_slice()),
                        content_id,
                        marked_content,
                        context,
                    );
                    if operation.operator == "BDC" {
                        let is_span = operation
                            .operands
                            .first()
                            .and_then(|operand| operand.as_name().ok())
                            == Some(b"Span".as_slice());
                        let actual_text_attribute_present = properties
                            .is_some_and(|dictionary| contains_key(dictionary, b"ActualText"));
                        let alt_text_attribute_present = properties
                            .and_then(|dictionary| dictionary.get(b"Alt").ok())
                            .and_then(|value| {
                                resolve_optional(
                                    self.document,
                                    value,
                                    self.limits.max_reference_depth,
                                )
                                .ok()
                                .flatten()
                            })
                            .is_some_and(|value| matches!(value, Object::String(_, _)));
                        let expansion_text_attribute_present = properties
                            .and_then(|dictionary| dictionary.get(b"E").ok())
                            .and_then(|value| {
                                resolve_optional(
                                    self.document,
                                    value,
                                    self.limits.max_reference_depth,
                                )
                                .ok()
                                .flatten()
                            })
                            .is_some_and(|value| matches!(value, Object::String(_, _)));
                        let actual_text_present = properties
                            .and_then(|dictionary| dictionary.get(b"ActualText").ok())
                            .is_some_and(|value| matches!(value, Object::String(_, _)));
                        let lang_present =
                            properties.is_some_and(|dictionary| contains_key(dictionary, b"Lang"));
                        let inherited_structure_language =
                            self.current_page_object_id.is_some_and(|page_id| {
                                marked_content.iter().any(|value| {
                                    value.mcid.is_some_and(|mcid| {
                                        self.tagged_text_language.contains(&(page_id, mcid))
                                    })
                                })
                            });
                        if is_span
                            && actual_text_attribute_present
                            && !lang_present
                            && !marked_content.iter().any(|value| value.lang_present)
                            && !inherited_structure_language
                        {
                            self.summary.span_actual_text_language_failures.push(
                                RuleFailure {
                                    object_id: content_id.map(Into::into),
                                    description: format!(
                                        "{context} Span marked content /ActualText has no local, inherited, or catalog /Lang"
                                    ),
                                },
                            );
                        }
                        if is_span
                            && alt_text_attribute_present
                            && !lang_present
                            && !marked_content.iter().any(|value| value.lang_present)
                            && !inherited_structure_language
                        {
                            self.summary.span_alt_text_language_failures.push(
                                RuleFailure {
                                    object_id: content_id.map(Into::into),
                                    description: format!(
                                        "{context} Span marked content /Alt has no local, inherited, or catalog /Lang"
                                    ),
                                },
                            );
                        }
                        if is_span
                            && expansion_text_attribute_present
                            && !lang_present
                            && !marked_content.iter().any(|value| value.lang_present)
                            && !inherited_structure_language
                        {
                            self.summary.span_expansion_text_language_failures.push(
                                RuleFailure {
                                    object_id: content_id.map(Into::into),
                                    description: format!(
                                        "{context} Span marked content /E has no local, inherited, or catalog /Lang"
                                    ),
                                },
                            );
                        }
                        let mcid = properties
                            .map(|dictionary| {
                                resolved_integer(
                                    self.document,
                                    dictionary,
                                    b"MCID",
                                    self.limits.max_reference_depth,
                                )
                            })
                            .transpose()?
                            .flatten();
                        if mcid.is_some() {
                            self.record_form_mcid();
                        }
                        let is_tagged_content = mcid
                            .map(|mcid| {
                                self.tagging.connection(
                                    self.document,
                                    self.limits,
                                    struct_parents,
                                    mcid,
                                )
                            })
                            .transpose()?
                            .flatten();
                        self.record_tagged_content_if_nested_in_artifact(
                            is_tagged_content == Some(true),
                            content_id,
                            marked_content,
                            context,
                        );
                        marked_content.push(MarkedContent {
                            actual_text_present,
                            lang_present,
                            mcid,
                            is_artifact: operation
                                .operands
                                .first()
                                .and_then(|operand| operand.as_name().ok())
                                == Some(b"Artifact".as_slice()),
                            is_tagged_content,
                        });
                    }
                    if let Some(dictionary) = properties
                        && let Some(failure) = crate::language::inspect_dictionary(
                            self.document,
                            self.limits,
                            dictionary,
                            None,
                            &format!("{context} marked-content property list"),
                        )
                    {
                        self.summary.language_failures.push(failure);
                    }
                    if let Some(dictionary) = properties
                        && let Some(failure) = crate::language::inspect_dictionary_pdfa23(
                            self.document,
                            self.limits,
                            dictionary,
                            None,
                            &format!("{context} marked-content property list"),
                        )
                    {
                        self.summary.language_failures_pdfa23.push(failure);
                    }
                    if let Some(dictionary) = properties
                        && let Some(failure) = crate::language::inspect_dictionary_pdfua1(
                            self.document,
                            self.limits,
                            dictionary,
                            None,
                            &format!("{context} marked-content property list"),
                        )
                    {
                        self.summary.language_failures_pdfua1.push(failure);
                    }
                }
                "EMC" => {
                    marked_content.pop();
                }
                "q" if operation.operands.is_empty() => {
                    if graphics_stack.len() >= self.limits.max_reference_depth {
                        return Err(PdfError::ReferenceDepth(self.limits.max_reference_depth));
                    }
                    graphics_stack.push(graphics_state.clone());
                    if graphics_stack.len() > 28 {
                        self.summary
                            .excessive_graphics_state_nesting
                            .push(RuleFailure {
                                object_id: None,
                                description: format!(
                                    "{context} reaches graphics-state nesting depth {}",
                                    graphics_stack.len()
                                ),
                            });
                    }
                }
                "Q" if operation.operands.is_empty() => {
                    if let Some(saved) = graphics_stack.pop() {
                        *graphics_state = saved;
                    }
                }
                "CS" | "cs" => {
                    if uncolored_type3_glyph {
                        continue;
                    }
                    let [value] = operation.operands.as_slice() else {
                        continue;
                    };
                    let Ok(_name) = value.as_name() else {
                        continue;
                    };
                    let selection = self.record_color_space(
                        value.clone(),
                        resources,
                        page_resources,
                        resources_are_inherited,
                        format!("{context}/{}", operation.operator),
                        inherited_resource_names,
                    )?;
                    if operation.operator == "CS" {
                        graphics_state.stroking_pattern = selection.is_pattern;
                        graphics_state.stroking_icc_cmyk = selection.is_icc_cmyk;
                        graphics_state.stroking_color_space_selected = true;
                    } else {
                        graphics_state.nonstroking_pattern = selection.is_pattern;
                        graphics_state.nonstroking_icc_cmyk = selection.is_icc_cmyk;
                        graphics_state.nonstroking_color_space_selected = true;
                    }
                }
                "g" | "G" => {
                    if uncolored_type3_glyph {
                        continue;
                    }
                    self.record_default_color_space(
                        b"DefaultGray",
                        b"DeviceGray",
                        resources,
                        page_resources,
                        context,
                    )?;
                    if operation.operator == "G" {
                        graphics_state.stroking_pattern = false;
                        graphics_state.stroking_color_space_selected = true;
                    } else {
                        graphics_state.nonstroking_pattern = false;
                        graphics_state.nonstroking_color_space_selected = true;
                    }
                }
                "rg" | "RG" => {
                    if uncolored_type3_glyph {
                        continue;
                    }
                    self.record_default_color_space(
                        b"DefaultRGB",
                        b"DeviceRGB",
                        resources,
                        page_resources,
                        context,
                    )?;
                    if operation.operator == "RG" {
                        graphics_state.stroking_pattern = false;
                        graphics_state.stroking_color_space_selected = true;
                    } else {
                        graphics_state.nonstroking_pattern = false;
                        graphics_state.nonstroking_color_space_selected = true;
                    }
                }
                "k" | "K" => {
                    if uncolored_type3_glyph {
                        continue;
                    }
                    self.record_default_color_space(
                        b"DefaultCMYK",
                        b"DeviceCMYK",
                        resources,
                        page_resources,
                        context,
                    )?;
                    if operation.operator == "K" {
                        graphics_state.stroking_pattern = false;
                        graphics_state.stroking_color_space_selected = true;
                    } else {
                        graphics_state.nonstroking_pattern = false;
                        graphics_state.nonstroking_color_space_selected = true;
                    }
                }
                "ri" => {
                    if let [operand] = operation.operands.as_slice()
                        && let Ok(name) = operand.as_name()
                    {
                        let name = String::from_utf8_lossy(name).into_owned();
                        if !is_standard_rendering_intent(&name) {
                            self.summary
                                .invalid_rendering_intents
                                .entry(name)
                                .or_insert_with(|| context.to_owned());
                        }
                    }
                }
                "gs" => {
                    let [operand] = operation.operands.as_slice() else {
                        continue;
                    };
                    let Ok(name) = operand.as_name() else {
                        continue;
                    };
                    let Some(value) = resource(
                        self.document,
                        self.limits,
                        resources,
                        page_resources,
                        resources_are_inherited,
                        b"ExtGState",
                        name,
                        Some(&mut *inherited_resource_names),
                    )?
                    else {
                        continue;
                    };
                    let key = resource_key(value, &format!("{context}/ExtGState"));
                    self.record_page_use(value);
                    let Some(dictionary) =
                        resolve_optional(self.document, value, self.limits.max_reference_depth)?
                            .and_then(|object| object.as_dict().ok())
                    else {
                        continue;
                    };
                    self.summary.extgstates.push(ExtGStateUse {
                        key,
                        dictionary: dictionary.clone(),
                    });
                    if self.extgstate_has_transparency(dictionary)? {
                        self.summary
                            .pages_with_transparency
                            .insert(self.current_page);
                    }
                    if let Some(value) = resolved_bool(
                        self.document,
                        dictionary,
                        b"OP",
                        self.limits.max_reference_depth,
                    )? {
                        graphics_state.stroking_overprint = value;
                    }
                    if let Some(value) = resolved_bool(
                        self.document,
                        dictionary,
                        b"op",
                        self.limits.max_reference_depth,
                    )? {
                        graphics_state.nonstroking_overprint = value;
                    }
                    if let Some(value) = resolved_integer(
                        self.document,
                        dictionary,
                        b"OPM",
                        self.limits.max_reference_depth,
                    )? {
                        graphics_state.overprint_mode = value;
                    }
                }
                "Tf" => {
                    let name = match operation.operands.as_slice() {
                        [name, _size] => name.as_name().ok(),
                        _ => None,
                    };
                    graphics_state.font = match name {
                        Some(name) => resource(
                            self.document,
                            self.limits,
                            resources,
                            page_resources,
                            resources_are_inherited,
                            b"Font",
                            name,
                            Some(&mut *inherited_resource_names),
                        )?
                        .map(|object| SelectedFont {
                            key: resource_key(object, &format!("{context}/Font")),
                            object: object.clone(),
                            description: describe_font(object, context, name),
                        }),
                        None => None,
                    };
                }
                "Tr" => {
                    if let [operand] = operation.operands.as_slice()
                        && let Ok(mode) = operand.as_i64()
                    {
                        graphics_state.rendering_mode = mode;
                    }
                }
                "Tj" | "TJ" | "'" | "\"" => {
                    let shown_bytes = crate::font_embedding::shown_text_bytes(&operation.operands);
                    if !shown_bytes.is_empty() {
                        let fills = matches!(graphics_state.rendering_mode, 0 | 2 | 4 | 6);
                        let strokes = matches!(graphics_state.rendering_mode, 1 | 2 | 5 | 6);
                        self.summary.uses_default_gray |= (fills
                            && !graphics_state.nonstroking_color_space_selected)
                            || (strokes && !graphics_state.stroking_color_space_selected);
                    }
                    if !shown_bytes.is_empty() && inspect_pdfua_content {
                        let language_is_determined =
                            marked_content.iter().any(|value| value.lang_present)
                                || self.current_page_object_id.is_some_and(|page_id| {
                                    marked_content.iter().any(|value| {
                                        value.mcid.is_some_and(|mcid| {
                                            self.tagged_text_language.contains(&(page_id, mcid))
                                        })
                                    })
                                });
                        if !language_is_determined {
                            self.summary.text_language_failures.push(RuleFailure {
                                object_id: content_id.map(Into::into),
                                description: format!(
                                    "{context} text has no local, inherited, or catalog /Lang"
                                ),
                            });
                        }
                        self.record_untagged_content(content_id, marked_content, context, "text");
                    }
                    if !shown_bytes.is_empty()
                        && let Some(font) = graphics_state.font.clone()
                    {
                        self.record_font_use(
                            &font,
                            graphics_state.rendering_mode,
                            &shown_bytes,
                            marked_content,
                        );
                        self.execute_type3_glyphs(
                            &font,
                            &shown_bytes,
                            page_resources,
                            graphics_state,
                            uncolored_type3_glyph,
                            marked_content,
                            active_forms,
                            decoded_bytes,
                            depth + 1,
                        )?;
                    }
                }
                "Do" => {
                    let [operand] = operation.operands.as_slice() else {
                        continue;
                    };
                    let Ok(name) = operand.as_name() else {
                        continue;
                    };
                    let Some(value) = resource(
                        self.document,
                        self.limits,
                        resources,
                        page_resources,
                        resources_are_inherited,
                        b"XObject",
                        name,
                        Some(&mut *inherited_resource_names),
                    )?
                    else {
                        continue;
                    };
                    self.execute_xobject(
                        value,
                        XObjectUseKind::Painted,
                        resource_context,
                        graphics_state,
                        marked_content,
                        active_forms,
                        decoded_bytes,
                        &format!("{context}/XObject /{}", String::from_utf8_lossy(name)),
                        depth + 1,
                    )?;
                }
                "sh" => {
                    if uncolored_type3_glyph {
                        continue;
                    }
                    let [operand] = operation.operands.as_slice() else {
                        continue;
                    };
                    let Ok(name) = operand.as_name() else {
                        continue;
                    };
                    let Some(shading) = resource(
                        self.document,
                        self.limits,
                        resources,
                        page_resources,
                        resources_are_inherited,
                        b"Shading",
                        name,
                        Some(&mut *inherited_resource_names),
                    )?
                    else {
                        continue;
                    };
                    self.record_page_use(shading);
                    let Some(shading) =
                        resolve_optional(self.document, shading, self.limits.max_reference_depth)?
                            .and_then(|object| object.as_dict().ok())
                    else {
                        continue;
                    };
                    if inspect_pdfua_content {
                        self.record_untagged_content(
                            content_id,
                            marked_content,
                            context,
                            "shading",
                        );
                    }
                    if let Ok(value) = shading.get(b"ColorSpace") {
                        let _ = self.record_color_space(
                            value.clone(),
                            resources,
                            page_resources,
                            resources_are_inherited,
                            format!("{context}/Shading /{}", String::from_utf8_lossy(name)),
                            inherited_resource_names,
                        )?;
                    }
                }
                "scn" | "SCN" => {
                    if uncolored_type3_glyph {
                        continue;
                    }
                    let pattern_selected = if operation.operator == "SCN" {
                        graphics_state.stroking_pattern
                    } else {
                        graphics_state.nonstroking_pattern
                    };
                    if !pattern_selected {
                        continue;
                    }
                    let Some(name) = operation
                        .operands
                        .last()
                        .and_then(|operand| operand.as_name().ok())
                    else {
                        continue;
                    };
                    let Some(pattern) = resource(
                        self.document,
                        self.limits,
                        resources,
                        page_resources,
                        resources_are_inherited,
                        b"Pattern",
                        name,
                        Some(&mut *inherited_resource_names),
                    )?
                    else {
                        continue;
                    };
                    self.execute_pattern(
                        pattern,
                        resource_context,
                        graphics_state,
                        marked_content,
                        active_forms,
                        decoded_bytes,
                        &format!("{context}/Pattern /{}", String::from_utf8_lossy(name)),
                        depth + 1,
                    )?;
                }
                _ if is_inline_image && operation.operands.is_empty() && inspect_pdfua_content => {
                    self.record_untagged_content(
                        content_id,
                        marked_content,
                        context,
                        "inline image",
                    );
                }
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*"
                    if operation.operands.is_empty() =>
                {
                    let stroke = matches!(
                        operation.operator.as_str(),
                        "S" | "s" | "B" | "B*" | "b" | "b*"
                    );
                    let fill = matches!(
                        operation.operator.as_str(),
                        "f" | "F" | "f*" | "B" | "B*" | "b" | "b*"
                    );
                    self.summary.uses_default_gray |= (fill
                        && !graphics_state.nonstroking_color_space_selected)
                        || (stroke && !graphics_state.stroking_color_space_selected);
                    if ((stroke
                        && graphics_state.stroking_icc_cmyk
                        && graphics_state.stroking_overprint)
                        || (fill
                            && graphics_state.nonstroking_icc_cmyk
                            && graphics_state.nonstroking_overprint))
                        && graphics_state.overprint_mode != 0
                    {
                        self.summary.icc_cmyk_overprint.push(RuleFailure {
                            object_id: None,
                            description: format!(
                                "{context} paints ICCBased CMYK with overprint mode {} and enabled overprinting",
                                graphics_state.overprint_mode
                            ),
                        });
                    }
                    if inspect_pdfua_content {
                        self.record_untagged_content(
                            content_id,
                            marked_content,
                            context,
                            "path painting",
                        );
                    }
                }
                _ => {}
            }
        }
        for (content_id, inherited_resource_names) in inherited_resources {
            if inherited_resource_names.is_empty() {
                continue;
            }
            self.summary.inherited_resources.push(RuleFailure {
                object_id: content_id.map(Into::into),
                description: format!(
                    "{context} refers to resources inherited from its page: {}",
                    inherited_resource_names
                        .iter()
                        .map(|name| format!("/{name}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            });
        }
        Ok(())
    }

    fn record_font_use(
        &mut self,
        font: &SelectedFont,
        rendering_mode: i64,
        shown_bytes: &[u8],
        marked_content: &[MarkedContent],
    ) {
        self.record_page_use(&font.object);
        let index = match self.font_indices.entry(font.key.clone()) {
            std::collections::btree_map::Entry::Occupied(entry) => *entry.get(),
            std::collections::btree_map::Entry::Vacant(entry) => {
                let index = self.summary.fonts.len();
                self.summary.fonts.push(FontUse {
                    key: font.key.clone(),
                    object: font.object.clone(),
                    description: font.description.clone(),
                    rendering_mode,
                    shown_bytes: Vec::new(),
                    text_runs: Vec::new(),
                });
                entry.insert(index);
                index
            }
        };
        let Some(usage) = self.summary.fonts.get_mut(index) else {
            return;
        };
        if rendering_mode != 3 {
            usage.shown_bytes.extend_from_slice(shown_bytes);
        }
        usage.text_runs.push(FontTextRun {
            shown_bytes: shown_bytes.to_vec(),
            actual_text_present: marked_content.iter().any(|value| value.actual_text_present),
            page_object_id: self.current_page_object_id,
            marked_content_id: marked_content.iter().rev().find_map(|value| value.mcid),
        });
    }

    fn record_artifact_if_nested(
        &mut self,
        is_artifact: bool,
        content_id: Option<ObjectId>,
        marked_content: &[MarkedContent],
        context: &str,
    ) {
        if is_artifact
            && marked_content
                .iter()
                .rev()
                .find_map(|content| content.is_tagged_content)
                .unwrap_or(false)
        {
            self.summary
                .artifacts_inside_tagged_content
                .push(RuleFailure {
                    object_id: content_id.map(Into::into),
                    description: format!(
                        "{context} contains /Artifact marked content inside tagged content"
                    ),
                });
        }
    }

    fn record_tagged_content_if_nested_in_artifact(
        &mut self,
        is_tagged_content: bool,
        content_id: Option<ObjectId>,
        marked_content: &[MarkedContent],
        context: &str,
    ) {
        if is_tagged_content && marked_content.iter().any(|content| content.is_artifact) {
            self.summary
                .tagged_content_inside_artifacts
                .push(RuleFailure {
                    object_id: content_id.map(Into::into),
                    description: format!(
                        "{context} contains tagged content inside /Artifact marked content"
                    ),
                });
        }
    }

    fn record_untagged_content(
        &mut self,
        content_id: Option<ObjectId>,
        marked_content: &[MarkedContent],
        context: &str,
        content_kind: &str,
    ) {
        if !marked_content.iter().any(|content| content.is_artifact)
            && !marked_content
                .iter()
                .rev()
                .find_map(|content| content.is_tagged_content)
                .unwrap_or(false)
        {
            self.summary.untagged_content.push(RuleFailure {
                object_id: content_id.map(Into::into),
                description: format!(
                    "{context} contains {content_kind} that is neither /Artifact nor tagged real content"
                ),
            });
        }
    }

    fn record_form_mcid(&mut self) {
        for object_id in &self.form_stack {
            if let Some(form) = self.summary.form_xobjects.get_mut(object_id) {
                form.contains_mcid = true;
            }
        }
    }

    fn execute_xobject(
        &mut self,
        object: &Object,
        kind: XObjectUseKind,
        resource_context: ResourceContext<'_>,
        graphics_state: &GraphicsState,
        marked_content: &mut Vec<MarkedContent>,
        active_forms: &mut BTreeSet<ObjectId>,
        decoded_bytes: &mut usize,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        let inspect_pdfua_content = resource_context.inspect_pdfua_content;
        let ResourceContext {
            resources,
            page_resources,
            resources_are_inherited,
            uncolored_type3_glyph,
            ..
        } = resource_context;
        if depth > self.limits.max_reference_depth {
            return Err(PdfError::ReferenceDepth(self.limits.max_reference_depth));
        }
        let key = resource_key(object, context);
        self.record_page_use(object);
        let object_id = object.as_reference().ok();
        let Some(resolved) =
            resolve_optional(self.document, object, self.limits.max_reference_depth)?
        else {
            return Ok(());
        };
        self.summary
            .xobjects
            .entry(key.clone())
            .or_insert_with(|| XObjectUse {
                key,
                object: resolved.clone(),
                painted: false,
                alternate: false,
                explicit_mask: false,
                soft_mask: false,
                appearance: false,
                occurrences: 0,
            })
            .record(kind);
        let Some(dictionary) = crate::object_resolution::dictionary_based(resolved) else {
            return Ok(());
        };
        let declared_subtype = resolved_name(
            self.document,
            dictionary,
            b"Subtype",
            self.limits.max_reference_depth,
        )?;
        let modeled_subtype = if kind == XObjectUseKind::Appearance && resolved.as_stream().is_ok()
        {
            Some(b"Form".as_slice())
        } else {
            declared_subtype
        };
        if modeled_subtype == Some(b"Form".as_slice())
            && resolved.as_stream().is_ok()
            && !object_id.is_some_and(|id| active_forms.contains(&id))
        {
            self.reserve_form_invocation()?;
        }
        if modeled_subtype == Some(b"Form".as_slice())
            && let Some(object_id) = object_id
        {
            self.summary
                .form_xobjects
                .entry(object_id)
                .or_default()
                .references += 1;
        }
        if modeled_subtype == Some(b"Form".as_slice())
            && self.transparency_group_present(dictionary)?
        {
            self.summary
                .pages_with_transparency
                .insert(self.current_page);
        }
        match modeled_subtype {
            Some(b"Image") => {
                if kind == XObjectUseKind::Painted && inspect_pdfua_content {
                    self.record_untagged_content(
                        object_id,
                        marked_content,
                        context,
                        "image XObject",
                    );
                }
                if let Some(value) = dictionary
                    .get(b"SMask")
                    .ok()
                    .map(|value| {
                        resolve_optional(self.document, value, self.limits.max_reference_depth)
                    })
                    .transpose()?
                    .flatten()
                    && !matches!(value, Object::Null)
                {
                    self.summary
                        .pages_with_transparency
                        .insert(self.current_page);
                }
                let is_stencil = dictionary
                    .get(b"ImageMask")
                    .ok()
                    .and_then(|value| value.as_bool().ok())
                    == Some(true);
                if matches!(kind, XObjectUseKind::Painted | XObjectUseKind::Alternate)
                    && !is_stencil
                    && let Ok(value) = dictionary.get(b"ColorSpace")
                {
                    let _ = self.record_color_space(
                        value.clone(),
                        resources,
                        page_resources,
                        resources_are_inherited,
                        context.to_owned(),
                        &mut BTreeSet::new(),
                    )?;
                }
                for (key, dependent_kind) in [
                    (b"Mask".as_slice(), XObjectUseKind::ExplicitMask),
                    (b"SMask".as_slice(), XObjectUseKind::SoftMask),
                ] {
                    if let Ok(dependent) = dictionary.get(key) {
                        self.execute_xobject(
                            dependent,
                            dependent_kind,
                            resource_context,
                            graphics_state,
                            marked_content,
                            active_forms,
                            decoded_bytes,
                            &format!("{context}/{}", String::from_utf8_lossy(key)),
                            depth + 1,
                        )?;
                    }
                }
                if let Ok(alternates) = dictionary.get(b"Alternates")
                    && let Some(alternates) = resolve_optional(
                        self.document,
                        alternates,
                        self.limits.max_reference_depth,
                    )?
                    .and_then(|object| object.as_array().ok())
                {
                    for (index, alternate) in alternates.iter().enumerate() {
                        let Some(alternate) = resolve_optional(
                            self.document,
                            alternate,
                            self.limits.max_reference_depth,
                        )?
                        .and_then(|object| object.as_dict().ok()) else {
                            continue;
                        };
                        if let Ok(image) = alternate.get(b"Image") {
                            self.execute_xobject(
                                image,
                                XObjectUseKind::Alternate,
                                resource_context,
                                graphics_state,
                                marked_content,
                                active_forms,
                                decoded_bytes,
                                &format!("{context}/Alternate {index}"),
                                depth + 1,
                            )?;
                        }
                    }
                }
            }
            Some(b"Form") => {
                if object_id.is_some_and(|id| !active_forms.insert(id)) {
                    return Ok(());
                }
                let form_resources = match dictionary.get(b"Resources") {
                    Ok(entry) => {
                        resolve_optional(self.document, entry, self.limits.max_reference_depth)?
                            .and_then(|object| object.as_dict().ok())
                            .cloned()
                    }
                    Err(_) => None,
                };
                self.record_group_color_space(
                    dictionary,
                    form_resources.as_ref(),
                    page_resources,
                    &format!("{context}/Group"),
                )?;
                let pushed_form = resolved.as_stream().is_ok() && object_id.is_some();
                let result = if resolved.as_stream().is_ok() {
                    if let Some(id) = object_id {
                        self.form_stack.push(id);
                    }
                    let mut form_graphics_state = graphics_state.clone();
                    let mut form_graphics_stack = Vec::new();
                    let mut form_marked_content = marked_content.clone();
                    self.execute_contents(
                        object,
                        ResourceContext {
                            struct_parents: resolved_integer(
                                self.document,
                                dictionary,
                                b"StructParents",
                                self.limits.max_reference_depth,
                            )?,
                            resources: form_resources.as_ref(),
                            page_resources,
                            resources_are_inherited: false,
                            inspect_pdfua_content,
                            uncolored_type3_glyph,
                        },
                        &mut form_graphics_state,
                        &mut form_graphics_stack,
                        &mut form_marked_content,
                        active_forms,
                        decoded_bytes,
                        context,
                        depth,
                    )
                } else {
                    Ok(())
                };
                if pushed_form {
                    self.form_stack.pop();
                }
                if let Some(id) = object_id {
                    active_forms.remove(&id);
                }
                result?;
            }
            _ => {}
        }
        Ok(())
    }

    fn reserve_form_invocation(&mut self) -> Result<(), PdfError> {
        if self.form_invocations >= self.limits.max_form_invocations {
            return Err(PdfError::FormInvocationLimit(
                self.limits.max_form_invocations,
            ));
        }
        self.form_invocations += 1;
        Ok(())
    }

    fn execute_pattern(
        &mut self,
        object: &Object,
        resource_context: ResourceContext<'_>,
        graphics_state: &GraphicsState,
        marked_content: &mut Vec<MarkedContent>,
        active_forms: &mut BTreeSet<ObjectId>,
        decoded_bytes: &mut usize,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        self.record_page_use(object);
        let ResourceContext {
            resources,
            page_resources,
            resources_are_inherited,
            uncolored_type3_glyph,
            ..
        } = resource_context;
        if depth > self.limits.max_reference_depth {
            return Err(PdfError::ReferenceDepth(self.limits.max_reference_depth));
        }
        let object_id = object.as_reference().ok();
        if object_id.is_some_and(|id| !active_forms.insert(id)) {
            return Ok(());
        }
        let Some(resolved) =
            resolve_optional(self.document, object, self.limits.max_reference_depth)?
        else {
            if let Some(id) = object_id {
                active_forms.remove(&id);
            }
            return Ok(());
        };
        let Some(dictionary) = crate::object_resolution::dictionary_based(resolved) else {
            if let Some(id) = object_id {
                active_forms.remove(&id);
            }
            return Ok(());
        };
        // A tiling Pattern can inherit ordinary resource categories from the
        // page, while its color-space resources remain scoped to the Pattern.
        let pattern_page_resources = page_resources.map(|resources| {
            let mut resources = resources.clone();
            resources.remove(b"ColorSpace");
            resources
        });
        let pattern_type = resolved_integer(
            self.document,
            dictionary,
            b"PatternType",
            self.limits.max_reference_depth,
        )?;
        if pattern_type == Some(2) {
            self.execute_shading_pattern(
                dictionary,
                resources,
                pattern_page_resources.as_ref(),
                resources_are_inherited,
                context,
            )?;
            if let Some(id) = object_id {
                active_forms.remove(&id);
            }
            return Ok(());
        }
        let Ok(_stream) = resolved.as_stream() else {
            if let Some(id) = object_id {
                active_forms.remove(&id);
            }
            return Ok(());
        };
        if pattern_type != Some(1) {
            if let Some(id) = object_id {
                active_forms.remove(&id);
            }
            return Ok(());
        }
        let pattern_resources = match dictionary.get(b"Resources") {
            Ok(entry) => resolve_optional(self.document, entry, self.limits.max_reference_depth)?
                .and_then(|object| object.as_dict().ok())
                .cloned(),
            Err(_) => None,
        };
        let result = self.execute_contents(
            object,
            ResourceContext {
                struct_parents: None,
                resources: pattern_resources.as_ref(),
                page_resources: pattern_page_resources.as_ref(),
                resources_are_inherited: false,
                inspect_pdfua_content: false,
                uncolored_type3_glyph,
            },
            &mut graphics_state.clone(),
            &mut Vec::new(),
            marked_content,
            active_forms,
            decoded_bytes,
            context,
            depth,
        );
        if let Some(id) = object_id {
            active_forms.remove(&id);
        }
        result
    }

    fn execute_shading_pattern(
        &mut self,
        pattern: &Dictionary,
        resources: Option<&Dictionary>,
        page_resources: Option<&Dictionary>,
        resources_are_inherited: bool,
        context: &str,
    ) -> Result<(), PdfError> {
        if let Ok(shading) = pattern.get(b"Shading")
            && let Some(shading) =
                resolve_optional(self.document, shading, self.limits.max_reference_depth)?
                    .and_then(crate::object_resolution::dictionary_based)
            && let Ok(color_space) = shading.get(b"ColorSpace")
        {
            let _ = self.record_color_space(
                color_space.clone(),
                resources,
                page_resources,
                resources_are_inherited,
                format!("{context}/Shading"),
                &mut BTreeSet::new(),
            )?;
        }
        Ok(())
    }

    fn execute_type3_glyphs(
        &mut self,
        selected: &SelectedFont,
        shown_bytes: &[u8],
        page_resources: Option<&Dictionary>,
        graphics_state: &GraphicsState,
        uncolored_type3_glyph: bool,
        marked_content: &mut Vec<MarkedContent>,
        active_forms: &mut BTreeSet<ObjectId>,
        decoded_bytes: &mut usize,
        depth: usize,
    ) -> Result<(), PdfError> {
        let Some(font) = resolve_optional(
            self.document,
            &selected.object,
            self.limits.max_reference_depth,
        )?
        .and_then(|object| object.as_dict().ok()) else {
            return Ok(());
        };
        let subtype = resolved_name(
            self.document,
            font,
            b"Subtype",
            self.limits.max_reference_depth,
        )?;
        if subtype != Some(b"Type3".as_slice()) {
            return Ok(());
        }
        let Some(char_procs) = font
            .get(b"CharProcs")
            .ok()
            .map(|value| resolve_optional(self.document, value, self.limits.max_reference_depth))
            .transpose()?
            .flatten()
            .and_then(|value| value.as_dict().ok())
        else {
            return Ok(());
        };
        let font_resources = font
            .get(b"Resources")
            .ok()
            .map(|value| resolve_optional(self.document, value, self.limits.max_reference_depth))
            .transpose()?
            .flatten()
            .and_then(|value| value.as_dict().ok())
            .cloned();
        for glyph_name in
            crate::font_embedding::type3_glyph_names(self.document, font, shown_bytes, self.limits)?
        {
            let Ok(char_proc) = char_procs.get(glyph_name.as_bytes()) else {
                continue;
            };
            let object_id = char_proc.as_reference().ok();
            if object_id.is_some_and(|id| !active_forms.insert(id)) {
                continue;
            }
            let result = self.execute_standalone_stream(
                char_proc,
                ResourceContext {
                    struct_parents: None,
                    resources: font_resources.as_ref(),
                    page_resources,
                    resources_are_inherited: false,
                    inspect_pdfua_content: false,
                    uncolored_type3_glyph,
                },
                graphics_state,
                marked_content,
                active_forms,
                decoded_bytes,
                &format!("{}/CharProc /{glyph_name}", selected.description),
                depth,
            );
            if let Some(id) = object_id {
                active_forms.remove(&id);
            }
            result?;
        }
        Ok(())
    }

    fn execute_standalone_stream(
        &mut self,
        object: &Object,
        resource_context: ResourceContext<'_>,
        graphics_state: &GraphicsState,
        marked_content: &mut Vec<MarkedContent>,
        active_forms: &mut BTreeSet<ObjectId>,
        decoded_bytes: &mut usize,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        let Some(stream) =
            resolve_optional(self.document, object, self.limits.max_reference_depth)?
                .and_then(|object| object.as_stream().ok())
        else {
            return Ok(());
        };
        let resources = match stream.dict.get(b"Resources") {
            Ok(value) => resolve_optional(self.document, value, self.limits.max_reference_depth)?
                .and_then(|object| object.as_dict().ok())
                .cloned(),
            Err(_) => resource_context.resources.cloned(),
        };
        let bytes = decode_content_stream_cached(
            stream,
            object.as_reference().ok(),
            self.cache,
            self.limits,
            decoded_bytes,
        )?;
        self.execute_contents(
            object,
            ResourceContext {
                resources: resources.as_ref(),
                uncolored_type3_glyph: resource_context.uncolored_type3_glyph
                    || is_uncolored_type3_glyph(&bytes),
                ..resource_context
            },
            &mut graphics_state.clone(),
            &mut Vec::new(),
            marked_content,
            active_forms,
            decoded_bytes,
            context,
            depth,
        )
    }

    fn record_group_color_space(
        &mut self,
        owner: &Dictionary,
        resources: Option<&Dictionary>,
        page_resources: Option<&Dictionary>,
        context: &str,
    ) -> Result<(), PdfError> {
        let Ok(group) = owner.get(b"Group") else {
            return Ok(());
        };
        let Some(group) = resolve_optional(self.document, group, self.limits.max_reference_depth)?
            .and_then(|object| object.as_dict().ok())
        else {
            return Ok(());
        };
        if resolved_name(self.document, group, b"S", self.limits.max_reference_depth)?
            == Some(b"Transparency".as_slice())
        {
            self.summary
                .pages_with_transparency
                .insert(self.current_page);
        }
        if let Ok(color_space) = group.get(b"CS") {
            let _ = self.record_color_space(
                color_space.clone(),
                resources,
                page_resources,
                false,
                context.to_owned(),
                &mut BTreeSet::new(),
            )?;
        }
        Ok(())
    }

    fn extgstate_has_transparency(&self, dictionary: &Dictionary) -> Result<bool, PdfError> {
        for key in [b"CA".as_slice(), b"ca"] {
            if let Some(value) = dictionary
                .get(key)
                .ok()
                .map(|value| {
                    resolve_optional(self.document, value, self.limits.max_reference_depth)
                })
                .transpose()?
                .flatten()
                .and_then(|value| value.as_float().ok())
                && value < 1.0
            {
                return Ok(true);
            }
        }
        if let Some(value) = dictionary
            .get(b"SMask")
            .ok()
            .map(|value| resolve_optional(self.document, value, self.limits.max_reference_depth))
            .transpose()?
            .flatten()
            && !matches!(value, Object::Null)
            && value.as_name().ok() != Some(b"None".as_slice())
        {
            return Ok(true);
        }
        Ok(resolved_name(
            self.document,
            dictionary,
            b"BM",
            self.limits.max_reference_depth,
        )?
        .is_some_and(|name| !matches!(name, b"Normal" | b"Compatible")))
    }

    fn transparency_group_present(&self, owner: &Dictionary) -> Result<bool, PdfError> {
        let Some(group) = owner
            .get(b"Group")
            .ok()
            .map(|value| resolve_optional(self.document, value, self.limits.max_reference_depth))
            .transpose()?
            .flatten()
            .and_then(|value| value.as_dict().ok())
        else {
            return Ok(false);
        };
        Ok(
            resolved_name(self.document, group, b"S", self.limits.max_reference_depth)?
                == Some(b"Transparency".as_slice()),
        )
    }

    fn record_color_space(
        &mut self,
        value: Object,
        resources: Option<&Dictionary>,
        page_resources: Option<&Dictionary>,
        resources_are_inherited: bool,
        context: String,
        inherited_resource_names: &mut BTreeSet<String>,
    ) -> Result<ColorSpaceSelection, PdfError> {
        let value = self.resolve_color_space(
            value,
            resources,
            page_resources,
            resources_are_inherited,
            inherited_resource_names,
            0,
        )?;
        let is_pattern = match &value {
            Object::Name(name) => name == b"Pattern",
            Object::Array(items) => {
                items.first().and_then(|item| item.as_name().ok()) == Some(b"Pattern".as_slice())
            }
            _ => false,
        };
        let is_icc_cmyk = is_icc_cmyk(self.document, &value, self.limits)?;
        self.summary
            .selected_color_spaces
            .push(SelectedColorSpace { value, context });
        Ok(ColorSpaceSelection {
            is_pattern,
            is_icc_cmyk,
        })
    }

    fn resolve_color_space(
        &mut self,
        value: Object,
        resources: Option<&Dictionary>,
        page_resources: Option<&Dictionary>,
        resources_are_inherited: bool,
        inherited_resource_names: &mut BTreeSet<String>,
        depth: usize,
    ) -> Result<Object, PdfError> {
        if depth > self.limits.max_reference_depth {
            return Err(PdfError::ReferenceDepth(self.limits.max_reference_depth));
        }
        self.record_page_use(&value);
        let resolved = resolve_optional(self.document, &value, self.limits.max_reference_depth)?
            .cloned()
            .unwrap_or(value);
        if let Ok(name) = resolved.as_name() {
            let (canonical, default) = match name {
                b"DeviceGray" | b"G" => (
                    Some(b"DeviceGray".as_slice()),
                    Some(b"DefaultGray".as_slice()),
                ),
                b"DeviceRGB" | b"RGB" => (
                    Some(b"DeviceRGB".as_slice()),
                    Some(b"DefaultRGB".as_slice()),
                ),
                b"DeviceCMYK" | b"CMYK" => (
                    Some(b"DeviceCMYK".as_slice()),
                    Some(b"DefaultCMYK".as_slice()),
                ),
                _ => (None, None),
            };
            if let Some(default) = default
                && let Some(replacement) = resource(
                    self.document,
                    self.limits,
                    resources,
                    page_resources,
                    resources_are_inherited,
                    b"ColorSpace",
                    default,
                    None,
                )?
            {
                return self.resolve_color_space(
                    replacement.clone(),
                    resources,
                    page_resources,
                    resources_are_inherited,
                    inherited_resource_names,
                    depth + 1,
                );
            }
            if let Some(canonical) = canonical {
                return Ok(Object::Name(canonical.to_vec()));
            }
            if let Some(selected) = resource(
                self.document,
                self.limits,
                resources,
                page_resources,
                resources_are_inherited,
                b"ColorSpace",
                name,
                Some(inherited_resource_names),
            )? {
                if resolves_to_device_color_space(self.document, selected, self.limits)? {
                    inherited_resource_names.remove(&String::from_utf8_lossy(name).into_owned());
                }
                return self.resolve_color_space(
                    selected.clone(),
                    resources,
                    page_resources,
                    resources_are_inherited,
                    inherited_resource_names,
                    depth + 1,
                );
            }
            return Ok(resolved);
        }
        let Ok(items) = resolved.as_array() else {
            return Ok(resolved);
        };
        let mut items = items.clone();
        let kind = items.first().and_then(|item| item.as_name().ok());
        let nested_index = match kind {
            Some(b"Indexed") => Some(1),
            Some(b"Separation" | b"DeviceN") => Some(2),
            Some(b"Pattern") => None,
            _ => None,
        };
        if let Some(index) = nested_index
            && let Some(nested) = items.get(index).cloned()
            && let Some(slot) = items.get_mut(index)
        {
            *slot = self.resolve_color_space(
                nested,
                resources,
                page_resources,
                resources_are_inherited,
                inherited_resource_names,
                depth + 1,
            )?;
        }
        Ok(Object::Array(items))
    }

    fn record_default_color_space(
        &mut self,
        name: &[u8],
        fallback: &[u8],
        resources: Option<&Dictionary>,
        page_resources: Option<&Dictionary>,
        context: &str,
    ) -> Result<(), PdfError> {
        let value = resource(
            self.document,
            self.limits,
            resources,
            page_resources,
            false,
            b"ColorSpace",
            name,
            None,
        )?
        .cloned()
        .unwrap_or_else(|| Object::Name(fallback.to_vec()));
        self.record_color_space(
            value,
            resources,
            page_resources,
            false,
            format!("{context}/{}", String::from_utf8_lossy(name)),
            &mut BTreeSet::new(),
        )
        .map(|_| ())
    }
}

fn collect_content_object_limit_findings(
    value: &Object,
    object_id: Option<PdfObjectId>,
    context: &str,
    summary: &mut ContentExecutionSummary,
) {
    match value {
        Object::Integer(value) if !(*value >= -2_147_483_648 && *value <= 2_147_483_647) => {
            summary.out_of_range_integers.push(RuleFailure {
                object_id,
                description: format!(
                    "{context} contains integer value {value} outside the PDF/A range"
                ),
            });
        }
        Object::Real(value) if !value.is_finite() || value.abs() > 32_767.0 => {
            summary.out_of_range_reals.push(RuleFailure {
                object_id,
                description: format!(
                    "{context} contains real value {value} outside the PDF/A-1 range"
                ),
            });
        }
        Object::String(value, _) => {
            if value.len() > 65_535 {
                summary.overlong_strings.push(RuleFailure {
                    object_id,
                    description: format!("{context} contains a string of {} bytes", value.len()),
                });
            }
            if value.len() > 32_767 {
                summary.overlong_strings_pdfa_2.push(RuleFailure {
                    object_id,
                    description: format!("{context} contains a string of {} bytes", value.len()),
                });
            }
        }
        Object::Array(values) => {
            for value in values {
                collect_content_object_limit_findings(value, object_id, context, summary);
            }
        }
        Object::Dictionary(dictionary) => {
            for (_, value) in dictionary.iter() {
                collect_content_object_limit_findings(value, object_id, context, summary);
            }
        }
        _ => {}
    }
}

fn inspect_content_syntax_limits(
    bytes: &[u8],
    object_at: &impl Fn(usize) -> Option<ObjectId>,
    context: &str,
    summary: &mut ContentExecutionSummary,
) {
    let mut cursor = 0;
    let mut literal_depth = 0_usize;
    let mut literal_length = 0_usize;
    let mut literal_object_id = None;
    while cursor < bytes.len() {
        let Some(byte) = bytes.get(cursor).copied() else {
            break;
        };
        if literal_depth > 0 {
            literal_length = literal_length.saturating_add(1);
            if byte == b'\\' {
                cursor = cursor.saturating_add(2);
                literal_length = literal_length.saturating_add(1);
                continue;
            }
            if byte == b'(' {
                literal_depth += 1;
            } else if byte == b')' {
                literal_depth -= 1;
                if literal_depth == 0 {
                    if literal_length > 65_535 {
                        summary.overlong_strings.push(RuleFailure {
                            object_id: literal_object_id,
                            description: format!(
                                "{context} contains a literal string longer than 65535 bytes"
                            ),
                        });
                    }
                    if literal_length > 32_767 {
                        summary.overlong_strings_pdfa_2.push(RuleFailure {
                            object_id: literal_object_id,
                            description: format!(
                                "{context} contains a literal string longer than 32767 bytes"
                            ),
                        });
                    }
                }
            }
            cursor += 1;
            continue;
        }
        if byte == b'%' {
            while cursor < bytes.len() && !matches!(bytes.get(cursor).copied(), Some(b'\r' | b'\n'))
            {
                cursor += 1;
            }
            continue;
        }
        if byte == b'<' && bytes.get(cursor + 1) == Some(&b'<') {
            cursor += 2;
            continue;
        }
        if byte == b'<' {
            cursor += 1;
            let mut digit_count = 0;
            while cursor < bytes.len() && bytes.get(cursor) != Some(&b'>') {
                let Some(value) = bytes.get(cursor).copied() else {
                    break;
                };
                if !value.is_ascii_whitespace() {
                    digit_count += 1;
                    if !value.is_ascii_hexdigit() {
                        summary.has_non_hex_character = true;
                    }
                }
                cursor += 1;
            }
            summary.has_odd_hex_string |= digit_count % 2 != 0;
            cursor += usize::from(cursor < bytes.len());
            continue;
        }
        if byte == b'(' {
            literal_depth = 1;
            literal_length = 0;
            literal_object_id = object_at(cursor).map(Into::into);
        }
        cursor += 1;
    }
    let mut cursor = 0;
    while let Some((token, start, next)) = next_content_token(bytes, cursor) {
        match token {
            ContentToken::Name(name) if name.len() > 127 => {
                summary.overlong_names.push(RuleFailure {
                    object_id: object_at(start).map(Into::into),
                    description: format!(
                        "{context} contains a name of {} bytes, exceeding the 127-byte limit",
                        name.len()
                    ),
                });
            }
            ContentToken::Bare(value) => {
                let Ok(value) = std::str::from_utf8(&value) else {
                    cursor = next;
                    continue;
                };
                let Ok(value) = value.parse::<f64>() else {
                    cursor = next;
                    continue;
                };
                if value != 0.0 && value.is_finite() && value.abs() < 1.175e-38 {
                    summary.underflow_reals_pdfa_2.push(RuleFailure {
                        object_id: object_at(start).map(Into::into),
                        description: format!(
                            "{context} contains nonzero real value {value} below the PDF/A-2/3 minimum"
                        ),
                    });
                }
            }
            _ => {}
        }
        cursor = next;
    }
}

fn is_uncolored_type3_glyph(bytes: &[u8]) -> bool {
    let mut cursor = 0;
    let mut numeric_operands = 0;
    while let Some((token, _, next)) = next_content_token(bytes, cursor) {
        match token {
            ContentToken::Bare(value) if value == b"d1" => return numeric_operands == 6,
            ContentToken::Bare(value)
                if std::str::from_utf8(&value)
                    .ok()
                    .and_then(|value| value.parse::<f64>().ok())
                    .is_some() =>
            {
                numeric_operands += 1;
            }
            _ => return false,
        }
        cursor = next;
    }
    false
}

fn resolved_bool(
    document: &Document,
    dictionary: &Dictionary,
    key: &[u8],
    max_reference_depth: usize,
) -> Result<Option<bool>, PdfError> {
    let Ok(value) = dictionary.get(key) else {
        return Ok(None);
    };
    Ok(resolve_optional(document, value, max_reference_depth)?
        .and_then(|value| value.as_bool().ok()))
}

fn is_icc_cmyk(
    document: &Document,
    value: &Object,
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    let Some(value) = resolve_optional(document, value, limits.max_reference_depth)? else {
        return Ok(false);
    };
    let Ok(items) = value.as_array() else {
        return Ok(false);
    };
    if items.first().and_then(|item| item.as_name().ok()) != Some(b"ICCBased".as_slice()) {
        return Ok(false);
    }
    let Some(profile) = items.get(1) else {
        return Ok(false);
    };
    let Some(profile) = resolve_optional(document, profile, limits.max_reference_depth)? else {
        return Ok(false);
    };
    let Ok(stream) = profile.as_stream() else {
        return Ok(false);
    };
    resolved_integer(document, &stream.dict, b"N", limits.max_reference_depth).map(|n| n == Some(4))
}

fn resolves_to_device_color_space(
    document: &Document,
    object: &Object,
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    Ok(matches!(
        resolve_optional(document, object, limits.max_reference_depth)?
            .and_then(|object| object.as_name().ok()),
        Some(b"DeviceGray" | b"G" | b"DeviceRGB" | b"RGB" | b"DeviceCMYK" | b"CMYK")
    ))
}

fn resource_key(object: &Object, context: &str) -> ResourceKey {
    object.as_reference().ok().map_or_else(
        || ResourceKey::Direct(context.to_owned()),
        ResourceKey::Indirect,
    )
}

fn describe_font(object: &Object, context: &str, name: &[u8]) -> String {
    match object {
        Object::Reference(_) => "font object".to_owned(),
        _ => format!(
            "direct font /{} in {context}",
            String::from_utf8_lossy(name)
        ),
    }
}

fn resource<'a>(
    document: &'a Document,
    limits: &SafetyLimits,
    resources: Option<&'a Dictionary>,
    page_resources: Option<&'a Dictionary>,
    resources_are_inherited: bool,
    category: &[u8],
    name: &[u8],
    inherited_resource_names: Option<&mut BTreeSet<String>>,
) -> Result<Option<&'a Object>, PdfError> {
    if let Some(object) = resource_once(document, limits, resources, category, name)? {
        if resources_are_inherited && let Some(names) = inherited_resource_names {
            names.insert(String::from_utf8_lossy(name).into_owned());
        }
        return Ok(Some(object));
    }
    let object = resource_once(document, limits, page_resources, category, name)?;
    if object.is_some()
        && let Some(names) = inherited_resource_names
    {
        names.insert(String::from_utf8_lossy(name).into_owned());
    }
    Ok(object)
}

pub(crate) fn is_standard_rendering_intent(name: &str) -> bool {
    matches!(
        name,
        "RelativeColorimetric" | "AbsoluteColorimetric" | "Perceptual" | "Saturation"
    )
}

fn is_pdf_1_4_operator(operator: &str) -> bool {
    matches!(
        operator,
        "q" | "Q"
            | "cm"
            | "w"
            | "J"
            | "j"
            | "M"
            | "d"
            | "ri"
            | "i"
            | "gs"
            | "m"
            | "l"
            | "c"
            | "v"
            | "y"
            | "h"
            | "re"
            | "S"
            | "s"
            | "f"
            | "F"
            | "f*"
            | "B"
            | "B*"
            | "b"
            | "b*"
            | "n"
            | "W"
            | "W*"
            | "BT"
            | "ET"
            | "Tc"
            | "Tw"
            | "Tz"
            | "TL"
            | "Tf"
            | "Tr"
            | "Ts"
            | "Td"
            | "TD"
            | "Tm"
            | "T*"
            | "Tj"
            | "TJ"
            | "'"
            | "\""
            | "d0"
            | "d1"
            | "CS"
            | "cs"
            | "SC"
            | "SCN"
            | "sc"
            | "scn"
            | "G"
            | "g"
            | "RG"
            | "rg"
            | "K"
            | "k"
            | "sh"
            | "BI"
            | "ID"
            | "EI"
            | "Do"
            | "MP"
            | "DP"
            | "BMC"
            | "BDC"
            | "EMC"
            | "BX"
            | "EX"
    )
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct InlineImage {
    color_space: Option<Vec<u8>>,
    rendering_intent: Option<Vec<u8>>,
    has_lzw: bool,
    has_invalid_pdfa2_filter: bool,
    has_interpolate: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct InlineImageTokenization<'a> {
    ordinary_content: Cow<'a, [u8]>,
    images: Vec<InlineImage>,
    // Distinguish generated markers from identical operators in the source.
    markers: Vec<bool>,
    image_offsets: Vec<usize>,
    // Output/input offsets following replacements, for mapping diagnostics
    // back to the original Contents members without copying a map per byte.
    source_offsets: Vec<(usize, usize)>,
}

impl InlineImageTokenization<'_> {
    fn original_offset(&self, offset: usize) -> usize {
        let index = self
            .source_offsets
            .partition_point(|(output, _)| *output <= offset);
        index
            .checked_sub(1)
            .and_then(|index| self.source_offsets.get(index))
            .map_or(offset, |&(output, input)| input + (offset - output))
    }
}

fn next_operator_offset(bytes: &[u8], cursor: &mut usize, operator: &str) -> Option<usize> {
    while let Some((token, start, next)) = next_content_token(bytes, *cursor) {
        *cursor = next;
        if let ContentToken::Bare(token) = token
            && (token == operator.as_bytes()
                || (operator == "n" && matches!(token.as_slice(), b"d0" | b"d1")))
        {
            return Some(start);
        }
    }
    None
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ContentToken {
    Name(Vec<u8>),
    Bare(Vec<u8>),
    OpenArray,
    CloseArray,
    Other,
}

fn tokenize_inline_images(bytes: &[u8]) -> InlineImageTokenization<'_> {
    let mut ordinary_content = None;
    let mut images = Vec::new();
    let mut markers = Vec::new();
    let mut image_offsets = Vec::new();
    let mut source_offsets = Vec::new();
    let mut cursor = 0usize;
    let mut retained = 0usize;
    let mut array_depth = 0usize;
    while cursor < bytes.len() {
        let token_start = cursor;
        let Some((token, start, next)) = next_content_token(bytes, cursor) else {
            break;
        };
        cursor = next;
        match token {
            ContentToken::OpenArray => array_depth = array_depth.saturating_add(1),
            ContentToken::CloseArray => array_depth = array_depth.saturating_sub(1),
            ContentToken::Bare(token) if array_depth == 0 && token == b"BI" => {
                let mut image = InlineImage::default();
                let mut color_space_value = false;
                let mut rendering_intent_value = false;
                let mut filter_value = false;
                let mut interpolate_value = false;
                let mut filter_array_depth = None;
                while let Some((token, _, next)) = next_content_token(bytes, cursor) {
                    cursor = next;
                    match token {
                        ContentToken::Bare(token) if token == b"ID" => {
                            cursor = find_inline_image_end(bytes, cursor).unwrap_or(bytes.len());
                            ordinary_content
                                .get_or_insert_with(|| Vec::with_capacity(bytes.len()))
                                .extend_from_slice(
                                    bytes.get(retained..token_start).unwrap_or_default(),
                                );
                            let ordinary = ordinary_content
                                .as_mut()
                                .expect("ordinary content initialized");
                            ordinary.push(b' ');
                            source_offsets.push((ordinary.len(), start));
                            ordinary.extend_from_slice(INLINE_IMAGE_OPERATOR.as_bytes());
                            ordinary.push(b' ');
                            source_offsets.push((ordinary.len(), cursor));
                            retained = cursor;
                            images.push(image);
                            image_offsets.push(start);
                            markers.push(true);
                            break;
                        }
                        ContentToken::Name(name) if color_space_value => {
                            image.color_space = Some(name);
                            color_space_value = false;
                        }
                        ContentToken::Name(name) if rendering_intent_value => {
                            image.rendering_intent = Some(name);
                            rendering_intent_value = false;
                        }
                        ContentToken::Name(name) if filter_value => {
                            image.has_lzw |= is_lzw_filter(&name);
                            image.has_invalid_pdfa2_filter |= !is_pdfa2_inline_filter(&name);
                            filter_value = false;
                        }
                        ContentToken::OpenArray if filter_value => {
                            filter_array_depth = Some(1usize);
                            filter_value = false;
                        }
                        ContentToken::OpenArray if filter_array_depth.is_some() => {
                            filter_array_depth = filter_array_depth.map(|depth| depth + 1);
                        }
                        ContentToken::CloseArray if filter_array_depth.is_some() => {
                            filter_array_depth = filter_array_depth
                                .and_then(|depth| (depth > 1).then_some(depth - 1));
                        }
                        ContentToken::Name(name) if filter_array_depth.is_some() => {
                            image.has_lzw |= is_lzw_filter(&name);
                            image.has_invalid_pdfa2_filter |= !is_pdfa2_inline_filter(&name);
                        }
                        ContentToken::Bare(value) if interpolate_value => {
                            image.has_interpolate |= value == b"true";
                            interpolate_value = false;
                        }
                        ContentToken::Name(name) => {
                            color_space_value = matches!(name.as_slice(), b"CS" | b"ColorSpace");
                            rendering_intent_value = matches!(name.as_slice(), b"Intent");
                            filter_value = matches!(name.as_slice(), b"F" | b"Filter");
                            interpolate_value = matches!(name.as_slice(), b"I" | b"Interpolate");
                        }
                        _ => {
                            color_space_value = false;
                            rendering_intent_value = false;
                            filter_value = false;
                            interpolate_value = false;
                        }
                    }
                }
            }
            ContentToken::Bare(token)
                if array_depth == 0 && token == INLINE_IMAGE_OPERATOR.as_bytes() =>
            {
                markers.push(false);
            }
            _ => {}
        }
    }
    let ordinary_content = match ordinary_content {
        Some(mut ordinary_content) => {
            ordinary_content.extend_from_slice(bytes.get(retained..).unwrap_or_default());
            Cow::Owned(ordinary_content)
        }
        None => Cow::Borrowed(bytes),
    };
    InlineImageTokenization {
        ordinary_content,
        images,
        markers,
        image_offsets,
        source_offsets,
    }
}

fn is_lzw_filter(name: &[u8]) -> bool {
    matches!(name, b"LZW" | b"LZWDecode")
}

fn is_pdfa2_inline_filter(name: &[u8]) -> bool {
    matches!(
        name,
        b"ASCIIHexDecode"
            | b"ASCII85Decode"
            | b"FlateDecode"
            | b"RunLengthDecode"
            | b"CCITTFaxDecode"
            | b"DCTDecode"
            | b"AHx"
            | b"A85"
            | b"Fl"
            | b"RL"
            | b"CCF"
            | b"DCT"
    )
}

fn content_syntax_is_balanced(bytes: &[u8]) -> bool {
    let mut cursor = 0usize;
    let mut arrays = 0usize;
    let mut dictionaries = 0usize;
    while cursor < bytes.len() {
        let Some(byte) = bytes.get(cursor).copied() else {
            break;
        };
        match byte {
            b'%' => {
                while cursor < bytes.len()
                    && !matches!(bytes.get(cursor).copied(), Some(b'\n' | b'\r'))
                {
                    cursor += 1;
                }
            }
            b'(' => {
                cursor += 1;
                let mut depth = 1usize;
                while cursor < bytes.len() && depth > 0 {
                    let Some(byte) = bytes.get(cursor).copied() else {
                        break;
                    };
                    match byte {
                        b'\\' => cursor = cursor.saturating_add(2),
                        b'(' => {
                            depth += 1;
                            cursor += 1;
                        }
                        b')' => {
                            depth -= 1;
                            cursor += 1;
                        }
                        _ => cursor += 1,
                    }
                }
                if depth != 0 {
                    return false;
                }
            }
            b'<' if bytes.get(cursor + 1) == Some(&b'<') => {
                dictionaries += 1;
                cursor += 2;
            }
            b'>' if bytes.get(cursor + 1) == Some(&b'>') => {
                if dictionaries == 0 {
                    return false;
                }
                dictionaries -= 1;
                cursor += 2;
            }
            b'<' => {
                cursor += 1;
                while cursor < bytes.len() && bytes.get(cursor) != Some(&b'>') {
                    cursor += 1;
                }
                if cursor == bytes.len() {
                    return false;
                }
                cursor += 1;
            }
            b'[' => {
                arrays += 1;
                cursor += 1;
            }
            b']' => {
                if arrays == 0 {
                    return false;
                }
                arrays -= 1;
                cursor += 1;
            }
            _ => cursor += 1,
        }
    }
    arrays == 0 && dictionaries == 0
}

fn next_content_token(bytes: &[u8], mut cursor: usize) -> Option<(ContentToken, usize, usize)> {
    loop {
        while cursor < bytes.len()
            && bytes
                .get(cursor)
                .is_some_and(|byte| is_pdf_whitespace(*byte))
        {
            cursor += 1;
        }
        if bytes.get(cursor) == Some(&b'%') {
            while cursor < bytes.len() && !matches!(bytes.get(cursor).copied(), Some(b'\n' | b'\r'))
            {
                cursor += 1;
            }
            continue;
        }
        break;
    }
    let token_start = cursor;
    let byte = *bytes.get(cursor)?;
    match byte {
        b'/' => {
            cursor += 1;
            let start = cursor;
            while cursor < bytes.len()
                && bytes
                    .get(cursor)
                    .is_some_and(|byte| !is_pdf_delimiter_or_whitespace(*byte))
            {
                cursor += 1;
            }
            Some((
                ContentToken::Name(decode_pdf_name(bytes.get(start..cursor)?)),
                token_start,
                cursor,
            ))
        }
        b'[' => Some((ContentToken::OpenArray, token_start, cursor + 1)),
        b']' => Some((ContentToken::CloseArray, token_start, cursor + 1)),
        b'(' => {
            cursor += 1;
            let mut depth = 1usize;
            while cursor < bytes.len() && depth > 0 {
                let Some(byte) = bytes.get(cursor).copied() else {
                    break;
                };
                match byte {
                    b'\\' => cursor = cursor.saturating_add(2),
                    b'(' => {
                        depth += 1;
                        cursor += 1;
                    }
                    b')' => {
                        depth -= 1;
                        cursor += 1;
                    }
                    _ => cursor += 1,
                }
            }
            Some((ContentToken::Other, token_start, cursor.min(bytes.len())))
        }
        b'<' if bytes.get(cursor + 1) != Some(&b'<') => {
            cursor += 1;
            while cursor < bytes.len() && bytes.get(cursor) != Some(&b'>') {
                cursor += 1;
            }
            Some((
                ContentToken::Other,
                token_start,
                (cursor + 1).min(bytes.len()),
            ))
        }
        byte if is_pdf_delimiter_or_whitespace(byte) => {
            Some((ContentToken::Other, token_start, cursor + 1))
        }
        _ => {
            let start = cursor;
            while cursor < bytes.len()
                && bytes
                    .get(cursor)
                    .is_some_and(|byte| !is_pdf_delimiter_or_whitespace(*byte))
            {
                cursor += 1;
            }
            Some((
                ContentToken::Bare(bytes.get(start..cursor)?.to_vec()),
                token_start,
                cursor,
            ))
        }
    }
}

/// lopdf's content decoder tokenizes `d0` and `d1` as the line-dash operator
/// `d` followed by a numeric operand. They are distinct Type3 glyph-metrics
/// operators in PDF 1.4. The executor does not need their metric side effects,
/// so replace only those exact bare tokens with the operand-free no-op `n`
/// before decoding. Names, strings, hexadecimal strings, and comments are
/// preserved because `next_content_token` reports their complete spans.
fn normalize_digit_suffixed_operators(bytes: &[u8]) -> Cow<'_, [u8]> {
    let mut normalized = None;
    let mut cursor = 0usize;
    let mut copied = 0usize;
    while let Some((token, start, next)) = next_content_token(bytes, cursor) {
        if matches!(token, ContentToken::Bare(ref value) if matches!(value.as_slice(), b"d0" | b"d1"))
        {
            let normalized = normalized.get_or_insert_with(|| Vec::with_capacity(bytes.len()));
            normalized.extend_from_slice(bytes.get(copied..start).unwrap_or_default());
            normalized.push(b'n');
            copied = next;
        }
        cursor = next;
    }
    match normalized {
        Some(mut normalized) => {
            normalized.extend_from_slice(bytes.get(copied..).unwrap_or_default());
            Cow::Owned(normalized)
        }
        None => Cow::Borrowed(bytes),
    }
}

fn find_inline_image_end(bytes: &[u8], cursor: usize) -> Option<usize> {
    let mut candidate = cursor;
    while candidate + 1 < bytes.len() {
        if bytes.get(candidate) == Some(&b'E')
            && bytes.get(candidate + 1) == Some(&b'I')
            && is_pdf_boundary(candidate.checked_sub(1).and_then(|i| bytes.get(i).copied()))
            && is_pdf_boundary(bytes.get(candidate + 2).copied())
        {
            return Some(candidate + 2);
        }
        candidate += 1;
    }
    None
}

fn decode_pdf_name(bytes: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        if bytes.get(cursor) == Some(&b'#')
            && let Some(pair) = bytes.get(cursor + 1..cursor + 3)
            && let Ok(pair) = std::str::from_utf8(pair)
            && let Ok(byte) = u8::from_str_radix(pair, 16)
        {
            decoded.push(byte);
            cursor += 3;
        } else {
            let Some(byte) = bytes.get(cursor).copied() else {
                break;
            };
            decoded.push(byte);
            cursor += 1;
        }
    }
    decoded
}

fn is_pdf_delimiter_or_whitespace(byte: u8) -> bool {
    is_pdf_whitespace(byte)
        || matches!(
            byte,
            b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
        )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use lopdf::{Dictionary, Document, Object, Stream, dictionary};

    use super::{
        ContentCache, execute_content, is_pdf_boundary, normalize_digit_suffixed_operators,
        tokenize_inline_images,
    };
    use crate::SafetyLimits;
    use crate::page_tree::PageEntry;

    #[test]
    fn nul_byte_is_a_pdf_boundary() {
        // PDF32000-1:2008 §7.2.2 lists NUL among the six whitespace
        // characters, unlike `u8::is_ascii_whitespace`, which omits it — a
        // keyword immediately preceded or followed by NUL must still be
        // recognized as a standalone token.
        assert!(is_pdf_boundary(Some(0)));
    }

    #[test]
    fn end_of_buffer_is_a_pdf_boundary() {
        assert!(is_pdf_boundary(None));
    }

    #[test]
    fn an_ordinary_letter_is_not_a_pdf_boundary() {
        assert!(!is_pdf_boundary(Some(b'a')));
    }

    #[test]
    fn type3_metric_operators_are_normalized_without_touching_data() {
        let normalized = normalize_digit_suffixed_operators(
            b"1000 0 d0\n1 2 3 4 5 6 d1\n/d0 (d1) <6430> % d1\n",
        );
        assert_eq!(
            normalized.as_ref(),
            b"1000 0 n\n1 2 3 4 5 6 n\n/d0 (d1) <6430> % d1\n".as_slice()
        );
        assert!(matches!(
            normalize_digit_suffixed_operators(b"1000 0 d0\n"),
            std::borrow::Cow::Owned(_)
        ));
    }

    #[test]
    fn ordinary_content_borrows_when_no_inline_image_is_present() {
        let bytes = b"q 1 0 m 2 2 l S\n";
        let tokenized = tokenize_inline_images(bytes);
        assert!(matches!(
            tokenized.ordinary_content,
            std::borrow::Cow::Borrowed(_)
        ));
        assert_eq!(tokenized.ordinary_content, bytes.as_slice());
    }

    #[test]
    fn normalized_content_borrows_when_no_type3_metric_operator_is_present() {
        let bytes = b"/d0 (d1) <6430> q\n";
        let normalized = normalize_digit_suffixed_operators(bytes);
        assert!(matches!(normalized, std::borrow::Cow::Borrowed(_)));
        assert_eq!(normalized, bytes.as_slice());
    }

    #[test]
    fn inline_image_keywords_require_pdf_token_boundaries() {
        let tokenized = tokenize_inline_images(
            b"(BI /F /LZW ID x EI) Tj\nBIx /F /LZW ID x EI\n\
              BI /W 1 /H 1 /F /FlateDecode ID x EI\n",
        );
        assert_eq!(tokenized.images.len(), 1);
        assert!(!tokenized.images[0].has_lzw);
    }

    #[test]
    fn inline_image_dictionary_decodes_names_and_filter_arrays() {
        let tokenized = tokenize_inline_images(
            b"BI % comment\n/C#53 /C#53#31 /Filter [/AHx /LZW#44ecode] ID x EI\n",
        );
        assert_eq!(tokenized.images.len(), 1);
        assert_eq!(tokenized.images[0].color_space, Some(b"CS1".to_vec()));
        assert!(tokenized.images[0].has_lzw);
    }

    #[test]
    fn inline_image_data_false_ei_candidate_does_not_stop_tokenization() {
        let tokenized = tokenize_inline_images(b"BI /W 1 /H 1 ID abcEIx def EI\n1 2 MaiUnknown\n");
        assert_eq!(tokenized.images.len(), 1);
        assert_eq!(
            tokenized.ordinary_content.as_ref(),
            b" pageInlineImage \n1 2 MaiUnknown\n".as_slice()
        );
    }

    #[test]
    fn ordinary_operation_parsing_resumes_after_each_inline_image() {
        assert_eq!(
            tokenize_inline_images(b"q BI /W 1 /H 1 ID x EI Q BI /W 1 /H 1 ID y EI /Im Do")
                .ordinary_content
                .as_ref(),
            b"q pageInlineImage  Q pageInlineImage  /Im Do".as_slice()
        );
    }

    #[test]
    fn inline_image_ei_accepts_delimiter_boundaries() {
        assert_eq!(
            tokenize_inline_images(b"BI /W 1 /H 1 ID x EI/Q")
                .ordinary_content
                .as_ref(),
            b" pageInlineImage /Q".as_slice()
        );
    }

    #[test]
    fn ordinary_operations_execute_before_and_after_inline_images_and_masks() {
        for image in [
            b"BI /W 1 /H 1 /CS /RGB /BPC 8 ID \xff\x00\x00 EI\n".as_slice(),
            b"BI /W 1 /H 1 /IM true /BPC 1 ID \x80 EI\n".as_slice(),
        ] {
            let (mut document, page_id) = content_test_document(dictionary! {
                "Font" => dictionary! { "F1" => dictionary! {
                    "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
                } },
            });
            let contents = document.add_object(Stream::new(
                Dictionary::new(),
                [
                    b"1 0 0 rg BT /F1 12 Tf (before) Tj ET\n".as_slice(),
                    image,
                    b"0 G BT (after) Tj ET\n".as_slice(),
                ]
                .concat(),
            ));
            document
                .get_object_mut(page_id)
                .and_then(Object::as_dict_mut)
                .expect("page")
                .set("Contents", contents);

            let summary = execute_test_document(&document, page_id).expect("execute inline image");
            assert_eq!(summary.fonts.len(), 1);
            assert_eq!(summary.fonts[0].shown_bytes, b"beforeafter");
            let ordinary_colors = summary
                .selected_color_spaces
                .iter()
                .filter(|selected| !selected.context.ends_with("/inline image"))
                .map(|selected| selected.value.as_name().expect("device colour space"))
                .collect::<Vec<_>>();
            assert_eq!(ordinary_colors, [b"DeviceRGB".as_slice(), b"DeviceGray"]);
            assert_eq!(summary.text_language_failures.len(), 2);
            assert_eq!(summary.untagged_content.len(), 3);
            assert!(summary.undefined_operators.is_empty());
        }
    }

    #[test]
    fn inline_image_tagging_uses_marked_content_at_each_image_position() {
        let (mut document, page_id) = content_test_document(Dictionary::new());
        let contents = document.add_object(Stream::new(
            Dictionary::new(),
            b"/Artifact BMC BI /W 1 /H 1 /IM true /BPC 1 ID \x80 EI EMC\n\
              BI /W 1 /H 1 /CS /RGB /BPC 8 /F /LZW /I true /Intent /Invalid ID x EI\n\
              /Artifact BMC BI /W 1 /H 1 /IM true /BPC 1 ID \x80 EI EMC\n"
                .to_vec(),
        ));
        document
            .get_object_mut(page_id)
            .and_then(Object::as_dict_mut)
            .expect("page")
            .set("Contents", contents);

        let summary = execute_test_document(&document, page_id).expect("execute inline images");
        assert_eq!(summary.untagged_content.len(), 1);
        assert!(
            summary.untagged_content[0]
                .description
                .contains("inline image")
        );
        assert!(summary.inline_image_lzw_context.is_some());
        assert!(summary.inline_image_invalid_filter_context.is_some());
        assert!(summary.inline_image_interpolate_context.is_some());
        assert!(summary.invalid_rendering_intents.contains_key("Invalid"));
        assert_eq!(summary.selected_color_spaces.len(), 1);
        assert_eq!(
            summary.selected_color_spaces[0]
                .value
                .as_name()
                .expect("inline image colour space"),
            b"DeviceRGB"
        );
        assert!(summary.undefined_operators.is_empty());
    }

    #[test]
    fn source_operators_cannot_be_confused_with_inline_image_markers() {
        for image in [b"".as_slice(), b" BI /W 1 /H 1 /IM true ID x EI\n"] {
            let (mut document, page_id) = content_test_document(Dictionary::new());
            let contents = document.add_object(Stream::new(
                Dictionary::new(),
                [
                    image,
                    b"pageInlineImage pageInlineImagepageInlineImage\n".as_slice(),
                    image,
                ]
                .concat(),
            ));
            document
                .get_object_mut(page_id)
                .and_then(Object::as_dict_mut)
                .expect("page")
                .set("Contents", contents);

            let summary =
                execute_test_document(&document, page_id).expect("execute source markers");
            assert_eq!(summary.undefined_operators.len(), 2);
            assert!(summary.undefined_operators.contains_key("pageInlineImage"));
            assert!(
                summary
                    .undefined_operators
                    .contains_key("pageInlineImagepageInlineImage")
            );
            assert_eq!(
                summary.untagged_content.len(),
                2 * usize::from(!image.is_empty())
            );
        }
    }

    #[test]
    fn page_content_arrays_execute_in_order() {
        let (mut document, page_id) = content_test_document(Dictionary::new());
        let rgb = document.add_object(Stream::new(Dictionary::new(), b"0 0 0 rg\n".to_vec()));
        let cmyk = document.add_object(Stream::new(Dictionary::new(), b"0 0 0 0 k\n".to_vec()));
        document
            .objects
            .get_mut(&page_id)
            .and_then(|object| object.as_dict_mut().ok())
            .expect("page")
            .set(
                "Contents",
                Object::Array(vec![Object::Reference(rgb), Object::Reference(cmyk)]),
            );
        let summary = execute_test_document(&document, page_id).expect("execute content");
        let names = summary
            .selected_color_spaces
            .iter()
            .map(|selected| selected.value.as_name().expect("device name"))
            .collect::<Vec<_>>();
        assert_eq!(names, vec![b"DeviceRGB".as_slice(), b"DeviceCMYK"]);
    }

    #[test]
    fn split_operands_preserve_font_marked_content_and_saved_graphics_state() {
        let (mut document, page_id) = content_test_document(dictionary! {
            "Font" => dictionary! { "F1" => dictionary! {
                "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
            } },
        });
        let chunks = [
            b"/Artifact <</Lang (en)>> BDC q BT /F1 ".as_slice(),
            b"12 Tf [(A)",
            b"(B)]",
            b"TJ (C) Tj ET Q EMC BT (D) Tj ET",
        ];
        let streams = chunks.map(|bytes| {
            let mut stream = Stream::new(Dictionary::new(), bytes.to_vec());
            stream.compress().expect("compress content");
            document.add_object(stream)
        });
        let contents = document.add_object(Object::Array(
            streams.iter().copied().map(Object::Reference).collect(),
        ));
        document
            .get_dictionary_mut(page_id)
            .expect("page")
            .set("Contents", contents);

        let summary = execute_test_document(&document, page_id).expect("execute split operations");
        assert_eq!(summary.fonts.len(), 1);
        assert_eq!(summary.fonts[0].shown_bytes, b"ABC");
        assert_eq!(summary.untagged_content.len(), 1);
        assert_eq!(
            summary.untagged_content[0].object_id,
            Some(streams[3].into())
        );
        assert_eq!(summary.text_language_failures.len(), 1);
        assert!(summary.undefined_operators.is_empty());
    }

    #[test]
    fn contents_boundaries_separate_tokens_and_comments_without_changing_strings() {
        let (mut document, page_id) = content_test_document(dictionary! {
            "Font" => dictionary! { "F1" => dictionary! {
                "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
            } },
        });
        let streams = [
            b"q % trailing comment".as_slice(),
            b"Q",
            b"q 0.5",
            b"",
            b"0.5 0.5 rg BT /F1",
            b"12 Tf [(A\\(B\\))",
            b"0 (C)]TJ ET Q",
        ]
        .map(|bytes| {
            Object::Reference(document.add_object(Stream::new(Dictionary::new(), bytes.to_vec())))
        });
        document
            .get_dictionary_mut(page_id)
            .expect("page")
            .set("Contents", streams.to_vec());

        let summary = execute_test_document(&document, page_id).expect("execute token boundaries");
        assert!(summary.undefined_operators.is_empty());
        assert_eq!(summary.fonts.len(), 1);
        assert_eq!(summary.fonts[0].shown_bytes, b"A(B)C");
        assert_eq!(summary.selected_color_spaces.len(), 1);
    }

    #[test]
    fn split_inline_images_preserve_operation_and_syntax_locations() {
        let (mut document, page_id) = content_test_document(Dictionary::new());
        let first = document.add_object(Stream::new(
            Dictionary::new(),
            b"BI /W 1 /H 1 /IM true ID x ".to_vec(),
        ));
        let second = document.add_object(Stream::new(
            Dictionary::new(),
            b"EI BT (text) Tj ET\n".to_vec(),
        ));
        let third = document.add_object(Stream::new(
            Dictionary::new(),
            [
                b"/".as_slice(),
                &[b'a'; 128],
                b" 0.0000000000000000000000000000000000000001 (more) Tj",
            ]
            .concat(),
        ));
        document.get_dictionary_mut(page_id).expect("page").set(
            "Contents",
            vec![Object::Reference(first), second.into(), third.into()],
        );

        let summary =
            execute_test_document(&document, page_id).expect("execute split inline image");
        let objects = summary
            .untagged_content
            .iter()
            .map(|failure| failure.object_id)
            .collect::<Vec<_>>();
        assert_eq!(
            objects,
            [Some(first.into()), Some(second.into()), Some(third.into())]
        );
        assert_eq!(summary.overlong_names[0].object_id, Some(third.into()));
        assert_eq!(
            summary.underflow_reals_pdfa_2[0].object_id,
            Some(third.into())
        );
    }

    #[test]
    fn joined_content_obeys_stream_and_total_limits_with_cached_members() {
        let (mut document, page_id) = content_test_document(Dictionary::new());
        let stream = document.add_object(Stream::new(Dictionary::new(), b"0 g ".to_vec()));
        for (copies, stream_limit, total_limit, succeeds) in
            [(3, 4, 14, true), (3, 3, 14, false), (4, 4, 14, false)]
        {
            document
                .get_dictionary_mut(page_id)
                .expect("page")
                .set("Contents", vec![Object::Reference(stream); copies]);
            let limits = SafetyLimits {
                max_decoded_stream_size: stream_limit,
                max_total_decoded_content_size: total_limit,
                ..SafetyLimits::default()
            };
            let result = execute_content(
                &document,
                &[PageEntry::Indirect(page_id)],
                &mut ContentCache::new(),
                &BTreeSet::new(),
                &limits,
                true,
            );
            let preflight =
                super::check_content_limits(&document, &[PageEntry::Indirect(page_id)], &limits);
            if succeeds {
                let summary = result.expect("joined bytes may exceed the individual stream limit");
                assert_eq!(summary.decoded_content_bytes, 4);
                assert_eq!(summary.selected_color_spaces.len(), copies);
                preflight.expect("preflight accepts bounded joined content");
            } else if stream_limit == 3 {
                assert!(matches!(
                    result,
                    Err(crate::PdfError::ContentDecodeLimit(3))
                ));
                assert!(matches!(
                    preflight,
                    Err(crate::PdfError::ContentDecodeLimit(3))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(crate::PdfError::TotalContentDecodeLimit(14))
                ));
                assert!(matches!(
                    preflight,
                    Err(crate::PdfError::TotalContentDecodeLimit(14))
                ));
            }
        }
    }

    #[test]
    fn form_resources_precede_page_resources() {
        let mut page_resources = dictionary! {
            "ColorSpace" => dictionary! {"CS1" => "DeviceRGB"},
        };
        let (mut document, page_id) = content_test_document(page_resources.clone());
        let form = document.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Form",
                "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                "Resources" => dictionary! {
                    "ColorSpace" => dictionary! {"CS1" => "DeviceCMYK"},
                },
            },
            b"/CS1 cs\n".to_vec(),
        ));
        page_resources.set("XObject", dictionary! {"Fm" => form});
        let contents = document.add_object(Stream::new(Dictionary::new(), b"/Fm Do\n".to_vec()));
        let page = document
            .objects
            .get_mut(&page_id)
            .and_then(|object| object.as_dict_mut().ok())
            .expect("page");
        page.set("Resources", page_resources);
        page.set("Contents", contents);

        let summary = execute_test_document(&document, page_id).expect("execute content");
        let selected = summary.selected_color_spaces.first().expect("selection");
        assert_eq!(
            selected
                .value
                .as_name()
                .expect("resolved Form colour space"),
            b"DeviceCMYK"
        );
    }

    #[test]
    fn active_form_cycle_terminates_but_repeated_invocations_remain_visible() {
        let (mut document, page_id) = content_test_document(Dictionary::new());
        let form_id = document.new_object_id();
        document.objects.insert(
            form_id,
            Object::Stream(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Resources" => dictionary! {
                        "XObject" => dictionary! {"Self" => form_id},
                    },
                },
                b"/Self Do\n".to_vec(),
            )),
        );
        let contents =
            document.add_object(Stream::new(Dictionary::new(), b"/Fm Do\n/Fm Do\n".to_vec()));
        let page = document
            .objects
            .get_mut(&page_id)
            .and_then(|object| object.as_dict_mut().ok())
            .expect("page");
        page.set(
            "Resources",
            dictionary! {"XObject" => dictionary! {"Fm" => form_id}},
        );
        page.set("Contents", contents);

        let summary = execute_test_document(&document, page_id).expect("execute cyclic Form");
        assert_eq!(
            summary
                .xobjects
                .get(&crate::object_resolution::ResourceKey::Indirect(form_id))
                .map(|use_| use_.occurrences),
            Some(4),
            "two ordinary invocations and their two cycle-edge observations remain visible"
        );
        assert_eq!(summary.xobjects.len(), 1);
    }

    #[test]
    fn active_pattern_cycle_terminates() {
        let (mut document, page_id) = content_test_document(Dictionary::new());
        let pattern_id = document.new_object_id();
        document.objects.insert(
            pattern_id,
            Object::Stream(Stream::new(
                dictionary! {
                    "Type" => "Pattern",
                    "PatternType" => 1,
                    "PaintType" => 1,
                    "TilingType" => 1,
                    "BBox" => vec![0.into(), 0.into(), 1.into(), 1.into()],
                    "XStep" => 1,
                    "YStep" => 1,
                    "Resources" => dictionary! {
                        "Pattern" => dictionary! {"Self" => pattern_id},
                    },
                },
                b"/Pattern cs\n/Self scn\nMaiUnknown\n".to_vec(),
            )),
        );
        let contents = document.add_object(Stream::new(
            Dictionary::new(),
            b"/Pattern cs\n/P1 scn\n".to_vec(),
        ));
        let page = document
            .objects
            .get_mut(&page_id)
            .and_then(|object| object.as_dict_mut().ok())
            .expect("page");
        page.set(
            "Resources",
            dictionary! {"Pattern" => dictionary! {"P1" => pattern_id}},
        );
        page.set("Contents", contents);

        let summary = execute_test_document(&document, page_id).expect("execute cyclic Pattern");
        assert_eq!(summary.undefined_operators.len(), 1);
        assert!(summary.undefined_operators.contains_key("MaiUnknown"));
    }

    #[test]
    fn cyclic_appearance_state_dictionary_terminates_at_the_single_state_layer() {
        let (mut document, page_id) = content_test_document(Dictionary::new());
        let states_id = document.new_object_id();
        document.objects.insert(
            states_id,
            Object::Dictionary(dictionary! {"Self" => states_id}),
        );
        let annotation = document.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Text",
            "Rect" => vec![0.into(), 0.into(), 1.into(), 1.into()],
            "AP" => dictionary! {"N" => states_id},
        });
        document
            .objects
            .get_mut(&page_id)
            .and_then(|object| object.as_dict_mut().ok())
            .expect("page")
            .set("Annots", vec![Object::Reference(annotation)]);
        let summary = execute_content(
            &document,
            &[PageEntry::Indirect(page_id)],
            &mut ContentCache::new(),
            &BTreeSet::new(),
            &SafetyLimits::default(),
            true,
        )
        .expect("cyclic nested appearance states stop after one state layer");
        assert!(summary.undefined_operators.is_empty());
    }

    #[test]
    fn cyclic_linked_image_graph_hits_the_reference_depth_limit() {
        let (mut document, page_id) = content_test_document(Dictionary::new());
        let image_id = document.new_object_id();
        document.objects.insert(
            image_id,
            Object::Stream(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 1,
                    "ImageMask" => true,
                    "Mask" => image_id,
                },
                vec![0],
            )),
        );
        let contents = document.add_object(Stream::new(Dictionary::new(), b"/Im Do\n".to_vec()));
        let page = document
            .objects
            .get_mut(&page_id)
            .and_then(|object| object.as_dict_mut().ok())
            .expect("page");
        page.set(
            "Resources",
            dictionary! {"XObject" => dictionary! {"Im" => image_id}},
        );
        page.set("Contents", contents);
        let limits = SafetyLimits {
            max_reference_depth: 3,
            ..SafetyLimits::default()
        };

        let error = execute_content(
            &document,
            &[PageEntry::Indirect(page_id)],
            &mut ContentCache::new(),
            &BTreeSet::new(),
            &limits,
            true,
        )
        .expect_err("cyclic linked images must be bounded");
        assert!(matches!(error, crate::PdfError::ReferenceDepth(3)));
    }

    #[test]
    fn active_type3_charproc_cycle_terminates() {
        let (mut document, page_id) = content_test_document(Dictionary::new());
        let font_id = document.new_object_id();
        let char_proc_id = document.new_object_id();
        document.objects.insert(
            char_proc_id,
            Object::Stream(Stream::new(
                Dictionary::new(),
                b"1000 0 d0\nBT\n/Self 12 Tf\n(A) Tj\nET\n".to_vec(),
            )),
        );
        document.objects.insert(
            font_id,
            Object::Dictionary(dictionary! {
                "Type" => "Font",
                "Subtype" => "Type3",
                "FontBBox" => vec![0.into(), 0.into(), 1000.into(), 1000.into()],
                "FontMatrix" => vec![
                    0.001.into(), 0.into(), 0.into(), 0.001.into(), 0.into(), 0.into(),
                ],
                "CharProcs" => dictionary! {"g1" => char_proc_id},
                "Encoding" => dictionary! {
                    "Type" => "Encoding",
                    "Differences" => vec![65.into(), Object::Name(b"g1".to_vec())],
                },
                "FirstChar" => 65,
                "LastChar" => 65,
                "Widths" => vec![1000.into()],
                "Resources" => dictionary! {"Font" => dictionary! {"Self" => font_id}},
            }),
        );
        let contents = document.add_object(Stream::new(
            Dictionary::new(),
            b"BT\n/T3 12 Tf\n(A) Tj\nET\n".to_vec(),
        ));
        let page = document
            .objects
            .get_mut(&page_id)
            .and_then(|object| object.as_dict_mut().ok())
            .expect("page");
        page.set(
            "Resources",
            dictionary! {"Font" => dictionary! {"T3" => font_id}},
        );
        page.set("Contents", contents);

        let summary = execute_test_document(&document, page_id).expect("execute cyclic Type3");
        assert_eq!(summary.fonts.len(), 1);
        assert_eq!(summary.fonts[0].text_runs.len(), 2);
    }

    #[test]
    fn nested_type3_d1_charproc_keeps_descendant_color_operations_uncolored() {
        let (mut document, page_id) = content_test_document(Dictionary::new());
        let inner_char_proc_id = document.add_object(Stream::new(
            Dictionary::new(),
            b"1000 0 d0\n/CS cs\n".to_vec(),
        ));
        let inner_font_id = document.new_object_id();
        document.objects.insert(
            inner_font_id,
            Object::Dictionary(dictionary! {
                "Type" => "Font",
                "Subtype" => "Type3",
                "FontBBox" => vec![0.into(), 0.into(), 1000.into(), 1000.into()],
                "FontMatrix" => vec![
                    0.001.into(), 0.into(), 0.into(), 0.001.into(), 0.into(), 0.into(),
                ],
                "CharProcs" => dictionary! {"inner" => inner_char_proc_id},
                "Encoding" => dictionary! {
                    "Type" => "Encoding",
                    "Differences" => vec![65.into(), Object::Name(b"inner".to_vec())],
                },
                "FirstChar" => 65,
                "LastChar" => 65,
                "Widths" => vec![1000.into()],
                "Resources" => dictionary! {
                    "ColorSpace" => dictionary! {"CS" => "DeviceCMYK"},
                },
            }),
        );
        let outer_char_proc_id = document.add_object(Stream::new(
            Dictionary::new(),
            b"1 2 3 4 5 6 d1\nBT\n/Inner 12 Tf\n(A) Tj\nET\n".to_vec(),
        ));
        let outer_font_id = document.new_object_id();
        document.objects.insert(
            outer_font_id,
            Object::Dictionary(dictionary! {
                "Type" => "Font",
                "Subtype" => "Type3",
                "FontBBox" => vec![0.into(), 0.into(), 1000.into(), 1000.into()],
                "FontMatrix" => vec![
                    0.001.into(), 0.into(), 0.into(), 0.001.into(), 0.into(), 0.into(),
                ],
                "CharProcs" => dictionary! {"outer" => outer_char_proc_id},
                "Encoding" => dictionary! {
                    "Type" => "Encoding",
                    "Differences" => vec![65.into(), Object::Name(b"outer".to_vec())],
                },
                "FirstChar" => 65,
                "LastChar" => 65,
                "Widths" => vec![1000.into()],
                "Resources" => dictionary! {
                    "Font" => dictionary! {"Inner" => inner_font_id},
                },
            }),
        );
        let contents = document.add_object(Stream::new(
            Dictionary::new(),
            b"BT\n/Outer 12 Tf\n(A) Tj\nET\n".to_vec(),
        ));
        let page = document
            .objects
            .get_mut(&page_id)
            .and_then(|object| object.as_dict_mut().ok())
            .expect("page");
        page.set(
            "Resources",
            dictionary! {
                "Font" => dictionary! {"Outer" => outer_font_id},
            },
        );
        page.set("Contents", contents);

        let summary = execute_test_document(&document, page_id).expect("execute nested Type3");
        assert!(summary.selected_color_spaces.is_empty());
    }

    #[test]
    fn deeply_nested_content_graph_obeys_the_reference_depth_limit() {
        let (mut document, page_id) = content_test_document(Dictionary::new());
        let mut child = None;
        for _ in 0..6 {
            let resources = child.map_or_else(Dictionary::new, |child| {
                dictionary! {"XObject" => dictionary! {"Next" => child}}
            });
            child = Some(document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 1.into(), 1.into()],
                    "Resources" => resources,
                },
                if child.is_some() {
                    b"/Next Do\n".to_vec()
                } else {
                    Vec::new()
                },
            )));
        }
        let contents = document.add_object(Stream::new(Dictionary::new(), b"/Root Do\n".to_vec()));
        let page = document
            .objects
            .get_mut(&page_id)
            .and_then(|object| object.as_dict_mut().ok())
            .expect("page");
        page.set(
            "Resources",
            dictionary! {"XObject" => dictionary! {"Root" => child.expect("form chain")}},
        );
        page.set("Contents", contents);
        let limits = SafetyLimits {
            max_reference_depth: 3,
            ..SafetyLimits::default()
        };
        let error = execute_content(
            &document,
            &[PageEntry::Indirect(page_id)],
            &mut ContentCache::new(),
            &BTreeSet::new(),
            &limits,
            true,
        )
        .expect_err("content graph depth limit");
        assert!(matches!(error, crate::PdfError::ReferenceDepth(3)));
    }

    #[test]
    fn repeated_shared_form_expansion_obeys_the_document_invocation_limit() {
        let (mut document, page_id) = content_test_document(Dictionary::new());
        let mut child = None;
        for _ in 0..6 {
            let resources = child.map_or_else(Dictionary::new, |child| {
                dictionary! {"XObject" => dictionary! {"Next" => child}}
            });
            child = Some(document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 1.into(), 1.into()],
                    "Resources" => resources,
                },
                if child.is_some() {
                    b"/Next Do\n/Next Do\n".to_vec()
                } else {
                    Vec::new()
                },
            )));
        }
        let contents = document.add_object(Stream::new(Dictionary::new(), b"/Root Do\n".to_vec()));
        let page = document
            .objects
            .get_mut(&page_id)
            .and_then(|object| object.as_dict_mut().ok())
            .expect("page");
        page.set(
            "Resources",
            dictionary! {"XObject" => dictionary! {"Root" => child.expect("form chain")}},
        );
        page.set("Contents", contents);
        let limits = SafetyLimits {
            max_form_invocations: 8,
            ..SafetyLimits::default()
        };

        let error = execute_content(
            &document,
            &[PageEntry::Indirect(page_id)],
            &mut ContentCache::new(),
            &BTreeSet::new(),
            &limits,
            true,
        )
        .expect_err("repeated shared-form expansion exceeds the document budget");

        assert!(matches!(error, crate::PdfError::FormInvocationLimit(8)));
    }

    #[test]
    fn individual_content_decode_limit_is_preserved() {
        let (mut document, page_id) = content_test_document(Dictionary::new());
        let contents = document.add_object(Stream::new(Dictionary::new(), vec![b'q'; 32]));
        document
            .objects
            .get_mut(&page_id)
            .and_then(|object| object.as_dict_mut().ok())
            .expect("page")
            .set("Contents", contents);
        let limits = SafetyLimits {
            max_decoded_stream_size: 16,
            ..SafetyLimits::default()
        };
        let error = execute_content(
            &document,
            &[PageEntry::Indirect(page_id)],
            &mut ContentCache::new(),
            &BTreeSet::new(),
            &limits,
            true,
        )
        .expect_err("decode limit");
        assert!(matches!(error, crate::PdfError::ContentDecodeLimit(16)));
    }

    #[test]
    fn total_content_decode_limit_spans_pages() {
        let (mut document, first_page) = content_test_document(Dictionary::new());
        let second_page = document.add_object(dictionary! {
            "Type" => "Page",
            "MediaBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
        });
        let first_contents = document.add_object(Stream::new(Dictionary::new(), vec![b'q'; 12]));
        let second_contents = document.add_object(Stream::new(Dictionary::new(), vec![b'Q'; 12]));
        for (page, contents) in [(first_page, first_contents), (second_page, second_contents)] {
            document
                .objects
                .get_mut(&page)
                .and_then(|object| object.as_dict_mut().ok())
                .expect("page")
                .set("Contents", contents);
        }
        let limits = SafetyLimits {
            max_decoded_stream_size: 16,
            max_total_decoded_content_size: 16,
            ..SafetyLimits::default()
        };

        let error = execute_content(
            &document,
            &[
                PageEntry::Indirect(first_page),
                PageEntry::Indirect(second_page),
            ],
            &mut ContentCache::new(),
            &BTreeSet::new(),
            &limits,
            true,
        )
        .expect_err("total decoded content limit");
        assert!(matches!(
            error,
            crate::PdfError::TotalContentDecodeLimit(16)
        ));
    }

    #[test]
    fn cached_content_is_counted_once_across_pages() {
        let (mut document, first_page) = content_test_document(Dictionary::new());
        let second_page = document.add_object(dictionary! {
            "Type" => "Page",
            "MediaBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
        });
        let contents = document.add_object(Stream::new(Dictionary::new(), vec![b'q'; 12]));
        for page in [first_page, second_page] {
            document
                .objects
                .get_mut(&page)
                .and_then(|object| object.as_dict_mut().ok())
                .expect("page")
                .set("Contents", contents);
        }
        let limits = SafetyLimits {
            max_decoded_stream_size: 16,
            max_total_decoded_content_size: 16,
            ..SafetyLimits::default()
        };

        execute_content(
            &document,
            &[
                PageEntry::Indirect(first_page),
                PageEntry::Indirect(second_page),
            ],
            &mut ContentCache::new(),
            &BTreeSet::new(),
            &limits,
            true,
        )
        .expect("cached content fits the total decode budget once");
    }

    fn content_test_document(resources: Dictionary) -> (Document, (u32, u16)) {
        let mut document = Document::with_version("1.4");
        let page_id = document.add_object(dictionary! {
            "Type" => "Page",
            "MediaBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
            "Resources" => resources,
        });
        (document, page_id)
    }

    #[test]
    fn default_gray_text_use_follows_rendering_mode_and_shown_bytes() {
        for (mode, paints) in [
            (0, true),
            (1, true),
            (2, true),
            (3, false),
            (4, true),
            (5, true),
            (6, true),
            (7, false),
        ] {
            let (mut document, page_id) = content_test_document(Dictionary::new());
            let contents = document.add_object(Stream::new(
                Dictionary::new(),
                format!("BT {mode} Tr (A) Tj ET").into_bytes(),
            ));
            document
                .get_object_mut(page_id)
                .and_then(Object::as_dict_mut)
                .expect("page")
                .set("Contents", contents);

            let summary = execute_test_document(&document, page_id).expect("execute text");
            assert_eq!(summary.uses_default_gray, paints, "rendering mode {mode}");
        }

        let (mut document, page_id) = content_test_document(Dictionary::new());
        let contents =
            document.add_object(Stream::new(Dictionary::new(), b"BT 0 Tr () Tj ET".to_vec()));
        document
            .get_object_mut(page_id)
            .and_then(Object::as_dict_mut)
            .expect("page")
            .set("Contents", contents);

        let summary = execute_test_document(&document, page_id).expect("execute empty text");
        assert!(!summary.uses_default_gray);
    }

    fn execute_test_document(
        document: &Document,
        page_id: (u32, u16),
    ) -> Result<super::ContentExecutionSummary, crate::PdfError> {
        execute_content(
            document,
            &[PageEntry::Indirect(page_id)],
            &mut ContentCache::new(),
            &BTreeSet::new(),
            &SafetyLimits::default(),
            true,
        )
    }
}
