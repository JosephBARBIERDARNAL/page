use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::object_resolution::{
    ResourceKey, resolve_optional, resolved_integer, resolved_name, walk_inherited,
};
use crate::page_tree::PageEntry;
use crate::report::RuleFailure;

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
    pub(crate) selected_color_spaces: Vec<SelectedColorSpace>,
    pub(crate) xobjects: BTreeMap<ResourceKey, XObjectUse>,
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
    pub(crate) overlong_strings: Vec<RuleFailure>,
    pub(crate) overlong_strings_pdfa_2: Vec<RuleFailure>,
    pub(crate) language_failures: Vec<RuleFailure>,
    pub(crate) language_failures_pdfa23: Vec<RuleFailure>,
    pub(crate) artifacts_inside_tagged_content: Vec<RuleFailure>,
    pub(crate) tagged_content_inside_artifacts: Vec<RuleFailure>,
    pub(crate) untagged_content: Vec<RuleFailure>,
    pub(crate) uses_default_gray: bool,
    pub(crate) inherited_resources: Vec<RuleFailure>,
    pub(crate) icc_cmyk_overprint: Vec<RuleFailure>,
    pub(crate) pages_with_transparency: BTreeSet<u32>,
}

/// A byte is a PDF token boundary when it is absent (end of buffer), one of
/// the six PDF32000 whitespace characters (`NUL`, HT, LF, FF, CR, SP — a
/// superset of `u8::is_ascii_whitespace`'s five, since the ASCII definition
/// omits `NUL`), or one of the nine PDF delimiter characters.
pub(crate) fn is_pdf_boundary(byte: Option<u8>) -> bool {
    byte.is_none_or(|byte| {
        matches!(byte, 0 | 9 | 10 | 12 | 13 | 32) || b"()<>[]{}/%".contains(&byte)
    })
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
    mut visit: impl FnMut(u32, usize, Option<PdfObjectId>, &'a Object) -> Result<(), PdfError>,
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
            visit(page_number, index, object_id.map(Into::into), resolved)?;
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
    limits: &SafetyLimits,
) -> Result<ContentExecutionSummary, PdfError> {
    let mut executor = ContentExecutor {
        document,
        limits,
        cache,
        summary: ContentExecutionSummary::default(),
        font_indices: BTreeMap::new(),
        current_page: 0,
        current_page_object_id: None,
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
    Ok(executor.summary)
}

struct ContentExecutor<'a> {
    document: &'a Document,
    limits: &'a SafetyLimits,
    cache: &'a mut ContentCache,
    summary: ContentExecutionSummary,
    font_indices: BTreeMap<ResourceKey, usize>,
    current_page: u32,
    current_page_object_id: Option<ObjectId>,
}

#[derive(Clone, Copy)]
struct ResourceContext<'a> {
    resources: Option<&'a Dictionary>,
    page_resources: Option<&'a Dictionary>,
    resources_are_inherited: bool,
    inspect_pdfua_content: bool,
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
    mcid: Option<i64>,
    is_artifact: bool,
    is_tagged_content: bool,
}

impl ContentExecutor<'_> {
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
                    resources: resources.as_ref(),
                    page_resources: resources.as_ref(),
                    resources_are_inherited,
                    inspect_pdfua_content: true,
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
        for (annotation_index, annotation) in annotations.iter().enumerate() {
            let Some(annotation) =
                resolve_optional(self.document, annotation, self.limits.max_reference_depth)?
                    .and_then(|object| object.as_dict().ok())
            else {
                continue;
            };
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
                    resources: page_resources,
                    page_resources,
                    resources_are_inherited: false,
                    inspect_pdfua_content: false,
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
            resources,
            page_resources,
            resources_are_inherited,
            inspect_pdfua_content,
        } = resource_context;
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
                self.execute_contents(
                    item,
                    resource_context,
                    graphics_state,
                    graphics_stack,
                    marked_content,
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
        let mut inherited_resource_names = BTreeSet::new();
        let inline_images = tokenize_inline_images(&bytes);
        let ordinary_content = inline_images.ordinary_content.as_ref();
        for inline in &inline_images.images {
            if let Some(name) = &inline.color_space {
                let _ = self.record_color_space(
                    Object::Name(name.clone()),
                    resources,
                    page_resources,
                    resources_are_inherited,
                    format!("{context}/inline image"),
                    &mut inherited_resource_names,
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
        inspect_content_syntax_limits(
            ordinary_content,
            content_id.map(Into::into),
            context,
            &mut self.summary,
        );
        let normalized_content = normalize_digit_suffixed_operators(ordinary_content);
        let Ok(content) = Content::decode(normalized_content.as_ref()) else {
            return Ok(());
        };
        for operation in content.operations {
            for operand in &operation.operands {
                collect_content_object_limit_findings(
                    operand,
                    content_id.map(Into::into),
                    &format!("{context}/{}", operation.operator),
                    &mut self.summary,
                );
            }
            if !is_pdf_1_4_operator(&operation.operator) {
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
                        mcid: None,
                        is_artifact: operation
                            .operands
                            .first()
                            .and_then(|operand| operand.as_name().ok())
                            == Some(b"Artifact".as_slice()),
                        is_tagged_content: false,
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
                            Some(&mut inherited_resource_names),
                        )?
                    } else {
                        Some(properties)
                    };
                    let properties = properties.and_then(|properties| {
                        resolve_optional(self.document, properties, self.limits.max_reference_depth)
                            .ok()
                            .flatten()
                            .and_then(|object| object.as_dict().ok())
                    });
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
                        let actual_text_present = properties
                            .and_then(|dictionary| dictionary.get(b"ActualText").ok())
                            .is_some_and(|value| matches!(value, Object::String(_, _)));
                        let mcid = properties
                            .and_then(|dictionary| dictionary.get(b"MCID").ok())
                            .and_then(|value| value.as_i64().ok());
                        self.record_tagged_content_if_nested_in_artifact(
                            mcid.is_some(),
                            content_id,
                            marked_content,
                            context,
                        );
                        marked_content.push(MarkedContent {
                            actual_text_present,
                            mcid,
                            is_artifact: operation
                                .operands
                                .first()
                                .and_then(|operand| operand.as_name().ok())
                                == Some(b"Artifact".as_slice()),
                            is_tagged_content: mcid.is_some(),
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
                        &mut inherited_resource_names,
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
                        Some(&mut inherited_resource_names),
                    )?
                    else {
                        continue;
                    };
                    let key = resource_key(value, &format!("{context}/ExtGState"));
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
                            Some(&mut inherited_resource_names),
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
                    self.summary.uses_default_gray |=
                        !graphics_state.nonstroking_color_space_selected;
                    let shown_bytes = crate::font_embedding::shown_text_bytes(&operation.operands);
                    if !shown_bytes.is_empty() && inspect_pdfua_content {
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
                        Some(&mut inherited_resource_names),
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
                        Some(&mut inherited_resource_names),
                    )?
                    else {
                        continue;
                    };
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
                            &mut inherited_resource_names,
                        )?;
                    }
                }
                "scn" | "SCN" => {
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
                        Some(&mut inherited_resource_names),
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
                "BI" if operation.operands.is_empty() && inspect_pdfua_content => {
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
        if !inherited_resource_names.is_empty() {
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
        let usage = &mut self.summary.fonts[index];
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
                .any(|content| content.is_tagged_content)
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
        if !marked_content
            .iter()
            .any(|content| content.is_artifact || content.is_tagged_content)
        {
            self.summary.untagged_content.push(RuleFailure {
                object_id: content_id.map(Into::into),
                description: format!(
                    "{context} contains {content_kind} that is neither /Artifact nor tagged real content"
                ),
            });
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
            ..
        } = resource_context;
        if depth > self.limits.max_reference_depth {
            return Err(PdfError::ReferenceDepth(self.limits.max_reference_depth));
        }
        let key = resource_key(object, context);
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
                let result = if resolved.as_stream().is_ok() {
                    let mut form_graphics_state = graphics_state.clone();
                    let mut form_graphics_stack = Vec::new();
                    self.execute_contents(
                        object,
                        ResourceContext {
                            resources: form_resources.as_ref(),
                            page_resources,
                            resources_are_inherited: false,
                            inspect_pdfua_content,
                        },
                        &mut form_graphics_state,
                        &mut form_graphics_stack,
                        marked_content,
                        active_forms,
                        decoded_bytes,
                        context,
                        depth,
                    )
                } else {
                    Ok(())
                };
                if let Some(id) = object_id {
                    active_forms.remove(&id);
                }
                result?;
            }
            _ => {}
        }
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
        let ResourceContext {
            resources,
            page_resources,
            resources_are_inherited,
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
        // A tiling Pattern may fall back to page resources for ordinary
        // resource categories, but veraPDF does not inherit the page's
        // DefaultGray/DefaultRGB/DefaultCMYK mappings into the Pattern's
        // own color-space scope.
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
                resources: pattern_resources.as_ref(),
                page_resources: pattern_page_resources.as_ref(),
                resources_are_inherited: false,
                inspect_pdfua_content: false,
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
                font_resources.as_ref(),
                page_resources,
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
        fallback_resources: Option<&Dictionary>,
        page_resources: Option<&Dictionary>,
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
            Err(_) => fallback_resources.cloned(),
        };
        self.execute_contents(
            object,
            ResourceContext {
                resources: resources.as_ref(),
                page_resources,
                resources_are_inherited: false,
                inspect_pdfua_content: false,
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
        &self,
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
            // Pinned veraPDF 1.30.2 deliberately does not model the
            // underlying space of an uncoloured Pattern.
            Some(b"Pattern") => None,
            _ => None,
        };
        if let Some(index) = nested_index
            && let Some(nested) = items.get(index).cloned()
        {
            items[index] = self.resolve_color_space(
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
    object_id: Option<PdfObjectId>,
    context: &str,
    summary: &mut ContentExecutionSummary,
) {
    let mut cursor = 0;
    let mut literal_depth = 0_usize;
    let mut literal_length = 0_usize;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
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
                            object_id,
                            description: format!(
                                "{context} contains a literal string longer than 65535 bytes"
                            ),
                        });
                    }
                    if literal_length > 32_767 {
                        summary.overlong_strings_pdfa_2.push(RuleFailure {
                            object_id,
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
            while cursor < bytes.len() && !matches!(bytes[cursor], b'\r' | b'\n') {
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
            while cursor < bytes.len() && bytes[cursor] != b'>' {
                let value = bytes[cursor];
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
        }
        cursor += 1;
    }
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
        Object::Reference((number, generation)) => format!("font object {number} {generation}"),
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

fn is_standard_rendering_intent(name: &str) -> bool {
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
    let mut cursor = 0usize;
    let mut retained = 0usize;
    let mut array_depth = 0usize;
    while cursor < bytes.len() {
        let token_start = cursor;
        let Some((token, _, next)) = next_content_token(bytes, cursor) else {
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
                                .extend_from_slice(&bytes[retained..token_start]);
                            ordinary_content
                                .as_mut()
                                .expect("ordinary content initialized")
                                .extend_from_slice(b" BI ");
                            retained = cursor;
                            images.push(image);
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
            _ => {}
        }
    }
    let ordinary_content = match ordinary_content {
        Some(mut ordinary_content) => {
            ordinary_content.extend_from_slice(&bytes[retained..]);
            Cow::Owned(ordinary_content)
        }
        None => Cow::Borrowed(bytes),
    };
    InlineImageTokenization {
        ordinary_content,
        images,
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
        match bytes[cursor] {
            b'%' => {
                while cursor < bytes.len() && !matches!(bytes[cursor], b'\n' | b'\r') {
                    cursor += 1;
                }
            }
            b'(' => {
                cursor += 1;
                let mut depth = 1usize;
                while cursor < bytes.len() && depth > 0 {
                    match bytes[cursor] {
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
                while cursor < bytes.len() && bytes[cursor] != b'>' {
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
        while cursor < bytes.len() && is_pdf_whitespace(bytes[cursor]) {
            cursor += 1;
        }
        if bytes.get(cursor) == Some(&b'%') {
            while cursor < bytes.len() && !matches!(bytes[cursor], b'\n' | b'\r') {
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
            while cursor < bytes.len() && !is_pdf_delimiter_or_whitespace(bytes[cursor]) {
                cursor += 1;
            }
            Some((
                ContentToken::Name(decode_pdf_name(&bytes[start..cursor])),
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
                match bytes[cursor] {
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
            while cursor < bytes.len() && bytes[cursor] != b'>' {
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
            while cursor < bytes.len() && !is_pdf_delimiter_or_whitespace(bytes[cursor]) {
                cursor += 1;
            }
            Some((
                ContentToken::Bare(bytes[start..cursor].to_vec()),
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
            normalized.extend_from_slice(&bytes[copied..start]);
            normalized.push(b'n');
            copied = next;
        }
        cursor = next;
    }
    match normalized {
        Some(mut normalized) => {
            normalized.extend_from_slice(&bytes[copied..]);
            Cow::Owned(normalized)
        }
        None => Cow::Borrowed(bytes),
    }
}

fn find_inline_image_end(bytes: &[u8], cursor: usize) -> Option<usize> {
    let mut candidate = cursor;
    while candidate + 1 < bytes.len() {
        if bytes[candidate] == b'E'
            && bytes[candidate + 1] == b'I'
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
        if bytes[cursor] == b'#'
            && let Some(pair) = bytes.get(cursor + 1..cursor + 3)
            && let Ok(pair) = std::str::from_utf8(pair)
            && let Ok(byte) = u8::from_str_radix(pair, 16)
        {
            decoded.push(byte);
            cursor += 3;
        } else {
            decoded.push(bytes[cursor]);
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

fn is_pdf_whitespace(byte: u8) -> bool {
    matches!(byte, 0 | 9 | 10 | 12 | 13 | 32)
}

#[cfg(test)]
mod tests {

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
            b" BI \n1 2 MaiUnknown\n".as_slice()
        );
    }

    #[test]
    fn ordinary_operation_parsing_resumes_after_each_inline_image() {
        assert_eq!(
            tokenize_inline_images(b"q BI /W 1 /H 1 ID x EI Q BI /W 1 /H 1 ID y EI /Im Do")
                .ordinary_content
                .as_ref(),
            b"q BI  Q BI  /Im Do".as_slice()
        );
    }

    #[test]
    fn inline_image_ei_accepts_delimiter_boundaries() {
        assert_eq!(
            tokenize_inline_images(b"BI /W 1 /H 1 ID x EI/Q")
                .ordinary_content
                .as_ref(),
            b" BI /Q".as_slice()
        );
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
            &SafetyLimits::default(),
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
            &limits,
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
            &limits,
        )
        .expect_err("content graph depth limit");
        assert!(matches!(error, crate::PdfError::ReferenceDepth(3)));
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
            &limits,
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
            &limits,
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
            &limits,
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

    fn execute_test_document(
        document: &Document,
        page_id: (u32, u16),
    ) -> Result<super::ContentExecutionSummary, crate::PdfError> {
        execute_content(
            document,
            &[PageEntry::Indirect(page_id)],
            &mut ContentCache::new(),
            &SafetyLimits::default(),
        )
    }
}
