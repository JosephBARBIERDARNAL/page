use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;
use crate::object_resolution::{ResourceKey, resolve_optional, walk_inherited};
use crate::page_tree::PageEntry;

#[derive(Clone, Debug)]
pub(crate) struct SelectedColorSpace {
    pub(crate) value: Object,
    pub(crate) context: String,
}

#[derive(Clone, Debug)]
pub(crate) struct XObjectUse {
    pub(crate) key: ResourceKey,
    pub(crate) object: Object,
    pub(crate) kind: XObjectUseKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum XObjectUseKind {
    Painted,
    Alternate,
    ExplicitMask,
    SoftMask,
}

#[derive(Clone, Debug)]
pub(crate) struct ExtGStateUse {
    pub(crate) key: ResourceKey,
    pub(crate) dictionary: Dictionary,
}

/// Deterministic discoveries made while executing the bounded page/Form
/// content graph. Consumers apply their own rule predicates to this one shared
/// population instead of maintaining subtly different reachability walkers.
#[derive(Clone, Debug, Default)]
pub(crate) struct ContentExecutionSummary {
    pub(crate) selected_color_spaces: Vec<SelectedColorSpace>,
    pub(crate) xobjects: Vec<XObjectUse>,
    pub(crate) extgstates: Vec<ExtGStateUse>,
    pub(crate) invalid_rendering_intents: BTreeMap<String, String>,
    pub(crate) undefined_operators: BTreeMap<String, String>,
    pub(crate) inline_image_lzw_context: Option<String>,
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
    decoded_bytes: &mut usize,
) -> Result<Vec<u8>, PdfError> {
    let remaining = limits
        .max_decoded_stream_size
        .saturating_sub(*decoded_bytes);
    let bytes = match stream.decompressed_content_with_limit(remaining) {
        Ok(bytes) => bytes,
        Err(lopdf::Error::Decompress(lopdf::DecompressError::MemoryLimitExceeded { .. })) => {
            return Err(PdfError::ContentDecodeLimit(limits.max_decoded_stream_size));
        }
        Err(_) if stream.content.len() <= remaining => stream.content.clone(),
        Err(_) => return Err(PdfError::ContentDecodeLimit(limits.max_decoded_stream_size)),
    };
    if bytes.len() > remaining {
        return Err(PdfError::ContentDecodeLimit(limits.max_decoded_stream_size));
    }
    *decoded_bytes = decoded_bytes.saturating_add(bytes.len());
    Ok(bytes)
}

/// A page/Form content-stream decode cache keyed by the stream's own
/// indirect object id, shared across the several inspectors that otherwise
/// each independently decompress and re-tokenize the same page and Form
/// content.
pub(crate) type ContentCache = HashMap<ObjectId, Rc<[u8]>>;

/// Decodes `stream`'s content like [`decode_content_stream`], reusing a
/// previous decode of the same indirect object from `cache` instead of
/// decompressing it again. `object_id` is the reference that resolved to
/// `stream` (a page `/Contents` entry or an invoked Form XObject), if any;
/// a directly embedded stream has no id to key on and is decoded but not
/// cached, exactly as it always was.
///
/// A cache hit still runs the same `decoded_bytes` budget accounting as a
/// fresh decode would (using the cached length), so the configured
/// `max_decoded_stream_size` limit applies identically either way.
pub(crate) fn decode_content_stream_cached(
    stream: &Stream,
    object_id: Option<ObjectId>,
    cache: &mut ContentCache,
    limits: &SafetyLimits,
    decoded_bytes: &mut usize,
) -> Result<Rc<[u8]>, PdfError> {
    if let Some(object_id) = object_id
        && let Some(cached) = cache.get(&object_id)
    {
        let remaining = limits
            .max_decoded_stream_size
            .saturating_sub(*decoded_bytes);
        if cached.len() > remaining {
            return Err(PdfError::ContentDecodeLimit(limits.max_decoded_stream_size));
        }
        *decoded_bytes = decoded_bytes.saturating_add(cached.len());
        return Ok(Rc::clone(cached));
    }
    let bytes: Rc<[u8]> = Rc::from(decode_content_stream(stream, limits, decoded_bytes)?);
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
    pages: &'a BTreeMap<u32, PageEntry>,
    limits: &SafetyLimits,
    inspected: &mut BTreeSet<ObjectId>,
    mut visit: impl FnMut(u32, usize, Option<PdfObjectId>, &'a Object) -> Result<(), PdfError>,
) -> Result<(), PdfError> {
    for (&page_number, page_entry) in pages {
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
    pages: &BTreeMap<u32, PageEntry>,
    cache: &mut ContentCache,
    limits: &SafetyLimits,
) -> Result<ContentExecutionSummary, PdfError> {
    let mut executor = ContentExecutor {
        document,
        limits,
        cache,
        summary: ContentExecutionSummary::default(),
    };
    for (&page_number, page_entry) in pages {
        let page = page_entry
            .resolve(document)
            .ok_or(PdfError::UnexpectedObject("page is not a dictionary"))?;
        executor.execute_page(page_number, page)?;
    }
    Ok(executor.summary)
}

struct ContentExecutor<'a> {
    document: &'a Document,
    limits: &'a SafetyLimits,
    cache: &'a mut ContentCache,
    summary: ContentExecutionSummary,
}

#[derive(Clone, Debug, Default)]
struct ColorState {
    stroking_pattern: bool,
    nonstroking_pattern: bool,
    stack: Vec<(bool, bool)>,
}

impl ContentExecutor<'_> {
    fn execute_page(&mut self, page_number: u32, page: &Dictionary) -> Result<(), PdfError> {
        let resources = inherited_page_resources(self.document, page, self.limits)?.cloned();
        let mut active_forms = BTreeSet::new();
        let mut decoded_bytes = 0usize;
        let mut color_state = ColorState::default();
        self.record_group_color_space(
            page,
            resources.as_ref(),
            resources.as_ref(),
            &format!("page {page_number}/Group"),
        )?;
        if let Ok(contents) = page.get(b"Contents") {
            self.execute_contents(
                contents,
                resources.as_ref(),
                resources.as_ref(),
                &mut color_state,
                &mut active_forms,
                &mut decoded_bytes,
                &format!("page {page_number}"),
                0,
            )?;
        }
        self.execute_annotation_appearances(
            page_number,
            page,
            resources.as_ref(),
            &mut active_forms,
            &mut decoded_bytes,
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
            let color_state = ColorState::default();
            return self.execute_xobject(
                entry,
                XObjectUseKind::Painted,
                page_resources,
                page_resources,
                &color_state,
                active_forms,
                decoded_bytes,
                context,
                depth + 1,
            );
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

    #[allow(clippy::too_many_arguments)]
    fn execute_contents(
        &mut self,
        contents: &Object,
        resources: Option<&Dictionary>,
        page_resources: Option<&Dictionary>,
        color_state: &mut ColorState,
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
                self.execute_contents(
                    item,
                    resources,
                    page_resources,
                    color_state,
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
        let inline_images = tokenize_inline_images(&bytes);
        for inline in &inline_images.images {
            if let Some(name) = &inline.color_space {
                let _ = self.record_color_space(
                    Object::Name(name.clone()),
                    resources,
                    page_resources,
                    format!("{context}/inline image"),
                )?;
            }
            if inline.has_lzw {
                self.summary
                    .inline_image_lzw_context
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

        let Ok(content) = Content::decode(&inline_images.ordinary_content) else {
            return Ok(());
        };
        for operation in content.operations {
            if !is_pdf_1_4_operator(&operation.operator) {
                self.summary
                    .undefined_operators
                    .entry(operation.operator.clone())
                    .or_insert_with(|| context.to_owned());
            }
            match operation.operator.as_str() {
                "q" => color_state.stack.push((
                    color_state.stroking_pattern,
                    color_state.nonstroking_pattern,
                )),
                "Q" => {
                    if let Some((stroking, nonstroking)) = color_state.stack.pop() {
                        color_state.stroking_pattern = stroking;
                        color_state.nonstroking_pattern = nonstroking;
                    }
                }
                "CS" | "cs" => {
                    if let Some(value) = operation.operands.first() {
                        let pattern = self.record_color_space(
                            value.clone(),
                            resources,
                            page_resources,
                            format!("{context}/{}", operation.operator),
                        )?;
                        if operation.operator == "CS" {
                            color_state.stroking_pattern = pattern;
                        } else {
                            color_state.nonstroking_pattern = pattern;
                        }
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
                        color_state.stroking_pattern = false;
                    } else {
                        color_state.nonstroking_pattern = false;
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
                        color_state.stroking_pattern = false;
                    } else {
                        color_state.nonstroking_pattern = false;
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
                        color_state.stroking_pattern = false;
                    } else {
                        color_state.nonstroking_pattern = false;
                    }
                }
                "ri" => {
                    if let Some(name) = operation
                        .operands
                        .first()
                        .and_then(|operand| operand.as_name().ok())
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
                    let Some(name) = operation
                        .operands
                        .first()
                        .and_then(|operand| operand.as_name().ok())
                    else {
                        continue;
                    };
                    let Some(value) = resource(
                        self.document,
                        self.limits,
                        resources,
                        page_resources,
                        b"ExtGState",
                        name,
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
                }
                "Do" => {
                    let Some(name) = operation
                        .operands
                        .first()
                        .and_then(|operand| operand.as_name().ok())
                    else {
                        continue;
                    };
                    let Some(value) = resource(
                        self.document,
                        self.limits,
                        resources,
                        page_resources,
                        b"XObject",
                        name,
                    )?
                    else {
                        continue;
                    };
                    self.execute_xobject(
                        value,
                        XObjectUseKind::Painted,
                        resources,
                        page_resources,
                        color_state,
                        active_forms,
                        decoded_bytes,
                        &format!("{context}/XObject /{}", String::from_utf8_lossy(name)),
                        depth + 1,
                    )?;
                }
                "sh" => {
                    let Some(name) = operation
                        .operands
                        .first()
                        .and_then(|operand| operand.as_name().ok())
                    else {
                        continue;
                    };
                    let Some(shading) = resource(
                        self.document,
                        self.limits,
                        resources,
                        page_resources,
                        b"Shading",
                        name,
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
                    if let Ok(value) = shading.get(b"ColorSpace") {
                        let _ = self.record_color_space(
                            value.clone(),
                            resources,
                            page_resources,
                            format!("{context}/Shading /{}", String::from_utf8_lossy(name)),
                        )?;
                    }
                }
                "scn" | "SCN" => {
                    let pattern_selected = if operation.operator == "SCN" {
                        color_state.stroking_pattern
                    } else {
                        color_state.nonstroking_pattern
                    };
                    if !pattern_selected {
                        continue;
                    }
                    for name in operation
                        .operands
                        .iter()
                        .filter_map(|operand| operand.as_name().ok())
                    {
                        let Some(pattern) = resource(
                            self.document,
                            self.limits,
                            resources,
                            page_resources,
                            b"Pattern",
                            name,
                        )?
                        else {
                            continue;
                        };
                        self.execute_pattern(
                            pattern,
                            resources,
                            page_resources,
                            color_state,
                            active_forms,
                            decoded_bytes,
                            &format!("{context}/Pattern /{}", String::from_utf8_lossy(name)),
                            depth + 1,
                        )?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_xobject(
        &mut self,
        object: &Object,
        kind: XObjectUseKind,
        resources: Option<&Dictionary>,
        page_resources: Option<&Dictionary>,
        color_state: &ColorState,
        active_forms: &mut BTreeSet<ObjectId>,
        decoded_bytes: &mut usize,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
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
        self.summary.xobjects.push(XObjectUse {
            key,
            object: resolved.clone(),
            kind,
        });
        let Ok(stream) = resolved.as_stream() else {
            return Ok(());
        };
        match stream
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|value| value.as_name().ok())
        {
            Some(b"Image") => {
                let is_stencil = stream
                    .dict
                    .get(b"ImageMask")
                    .ok()
                    .and_then(|value| value.as_bool().ok())
                    == Some(true);
                if matches!(kind, XObjectUseKind::Painted | XObjectUseKind::Alternate)
                    && !is_stencil
                    && let Ok(value) = stream.dict.get(b"ColorSpace")
                {
                    let _ = self.record_color_space(
                        value.clone(),
                        resources,
                        page_resources,
                        context.to_owned(),
                    )?;
                }
                for (key, dependent_kind) in [
                    (b"Mask".as_slice(), XObjectUseKind::ExplicitMask),
                    (b"SMask".as_slice(), XObjectUseKind::SoftMask),
                ] {
                    if let Ok(dependent) = stream.dict.get(key) {
                        self.execute_xobject(
                            dependent,
                            dependent_kind,
                            resources,
                            page_resources,
                            color_state,
                            active_forms,
                            decoded_bytes,
                            &format!("{context}/{}", String::from_utf8_lossy(key)),
                            depth + 1,
                        )?;
                    }
                }
                if let Ok(alternates) = stream.dict.get(b"Alternates")
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
                                resources,
                                page_resources,
                                color_state,
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
                let form_resources = match stream.dict.get(b"Resources") {
                    Ok(entry) => {
                        resolve_optional(self.document, entry, self.limits.max_reference_depth)?
                            .and_then(|object| object.as_dict().ok())
                            .cloned()
                    }
                    Err(_) => None,
                };
                self.record_group_color_space(
                    &stream.dict,
                    form_resources.as_ref(),
                    page_resources,
                    &format!("{context}/Group"),
                )?;
                let mut form_color_state = color_state.clone();
                form_color_state.stack.clear();
                let result = self.execute_contents(
                    object,
                    form_resources.as_ref(),
                    page_resources,
                    &mut form_color_state,
                    active_forms,
                    decoded_bytes,
                    context,
                    depth,
                );
                if let Some(id) = object_id {
                    active_forms.remove(&id);
                }
                result?;
            }
            _ => {}
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_pattern(
        &mut self,
        object: &Object,
        resources: Option<&Dictionary>,
        page_resources: Option<&Dictionary>,
        color_state: &ColorState,
        active_forms: &mut BTreeSet<ObjectId>,
        decoded_bytes: &mut usize,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        if depth > self.limits.max_reference_depth {
            return Err(PdfError::ReferenceDepth(self.limits.max_reference_depth));
        }
        let object_id = object.as_reference().ok();
        if object_id.is_some_and(|id| !active_forms.insert(id)) {
            return Ok(());
        }
        let Some(stream) =
            resolve_optional(self.document, object, self.limits.max_reference_depth)?
                .and_then(|object| object.as_stream().ok())
        else {
            if let Some(id) = object_id {
                active_forms.remove(&id);
            }
            return Ok(());
        };
        if stream
            .dict
            .get(b"PatternType")
            .ok()
            .and_then(|value| value.as_i64().ok())
            != Some(1)
        {
            if let Some(id) = object_id {
                active_forms.remove(&id);
            }
            return Ok(());
        }
        let pattern_resources = match stream.dict.get(b"Resources") {
            Ok(entry) => resolve_optional(self.document, entry, self.limits.max_reference_depth)?
                .and_then(|object| object.as_dict().ok())
                .cloned(),
            Err(_) => None,
        };
        let result = self.execute_contents(
            object,
            pattern_resources.as_ref().or(resources),
            page_resources,
            &mut {
                let mut pattern_color_state = color_state.clone();
                pattern_color_state.stack.clear();
                pattern_color_state
            },
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
        if let Ok(color_space) = group.get(b"CS") {
            let _ = self.record_color_space(
                color_space.clone(),
                resources,
                page_resources,
                context.to_owned(),
            )?;
        }
        Ok(())
    }

    fn record_color_space(
        &mut self,
        value: Object,
        resources: Option<&Dictionary>,
        page_resources: Option<&Dictionary>,
        context: String,
    ) -> Result<bool, PdfError> {
        let value = self.resolve_color_space(value, resources, page_resources, 0)?;
        let is_pattern = match &value {
            Object::Name(name) => name == b"Pattern",
            Object::Array(items) => {
                items.first().and_then(|item| item.as_name().ok()) == Some(b"Pattern".as_slice())
            }
            _ => false,
        };
        self.summary
            .selected_color_spaces
            .push(SelectedColorSpace { value, context });
        Ok(is_pattern)
    }

    fn resolve_color_space(
        &self,
        value: Object,
        resources: Option<&Dictionary>,
        page_resources: Option<&Dictionary>,
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
                    b"ColorSpace",
                    default,
                )?
            {
                return self.resolve_color_space(
                    replacement.clone(),
                    resources,
                    page_resources,
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
                b"ColorSpace",
                name,
            )? {
                return self.resolve_color_space(
                    selected.clone(),
                    resources,
                    page_resources,
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
            // Pinned veraPDF 1.28.2 deliberately does not model the
            // underlying space of an uncoloured Pattern.
            Some(b"Pattern") => None,
            _ => None,
        };
        if let Some(index) = nested_index
            && let Some(nested) = items.get(index).cloned()
        {
            items[index] =
                self.resolve_color_space(nested, resources, page_resources, depth + 1)?;
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
            b"ColorSpace",
            name,
        )?
        .cloned()
        .unwrap_or_else(|| Object::Name(fallback.to_vec()));
        self.record_color_space(
            value,
            resources,
            page_resources,
            format!("{context}/{}", String::from_utf8_lossy(name)),
        )
        .map(|_| ())
    }
}

fn resource_key(object: &Object, context: &str) -> ResourceKey {
    object.as_reference().ok().map_or_else(
        || ResourceKey::Direct(context.to_owned()),
        ResourceKey::Indirect,
    )
}

fn resource<'a>(
    document: &'a Document,
    limits: &SafetyLimits,
    resources: Option<&'a Dictionary>,
    page_resources: Option<&'a Dictionary>,
    category: &[u8],
    name: &[u8],
) -> Result<Option<&'a Object>, PdfError> {
    if let Some(object) = resource_once(document, limits, resources, category, name)? {
        return Ok(Some(object));
    }
    resource_once(document, limits, page_resources, category, name)
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
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct InlineImageTokenization {
    ordinary_content: Vec<u8>,
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

fn tokenize_inline_images(bytes: &[u8]) -> InlineImageTokenization {
    let mut result = InlineImageTokenization {
        ordinary_content: Vec::with_capacity(bytes.len()),
        images: Vec::new(),
    };
    let mut cursor = 0usize;
    let mut retained = 0usize;
    let mut array_depth = 0usize;
    while cursor < bytes.len() {
        let token_start = cursor;
        let Some((token, next)) = next_content_token(bytes, cursor) else {
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
                let mut filter_array_depth = None;
                while let Some((token, next)) = next_content_token(bytes, cursor) {
                    cursor = next;
                    match token {
                        ContentToken::Bare(token) if token == b"ID" => {
                            cursor = find_inline_image_end(bytes, cursor).unwrap_or(bytes.len());
                            result
                                .ordinary_content
                                .extend_from_slice(&bytes[retained..token_start]);
                            result.ordinary_content.push(b' ');
                            retained = cursor;
                            result.images.push(image);
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
                        }
                        ContentToken::Name(name) => {
                            color_space_value = matches!(name.as_slice(), b"CS" | b"ColorSpace");
                            rendering_intent_value = matches!(name.as_slice(), b"Intent");
                            filter_value = matches!(name.as_slice(), b"F" | b"Filter");
                        }
                        _ => {
                            color_space_value = false;
                            rendering_intent_value = false;
                            filter_value = false;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    result
        .ordinary_content
        .extend_from_slice(&bytes[retained..]);
    result
}

/// Removes inline-image dictionaries and data before lopdf parses ordinary
/// content operations. Kept crate-private for the independent font walker.
pub(crate) fn content_without_inline_images(bytes: &[u8]) -> Vec<u8> {
    tokenize_inline_images(bytes).ordinary_content
}

fn is_lzw_filter(name: &[u8]) -> bool {
    matches!(name, b"LZW" | b"LZWDecode")
}

fn next_content_token(bytes: &[u8], mut cursor: usize) -> Option<(ContentToken, usize)> {
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
                cursor,
            ))
        }
        b'[' => Some((ContentToken::OpenArray, cursor + 1)),
        b']' => Some((ContentToken::CloseArray, cursor + 1)),
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
            Some((ContentToken::Other, cursor.min(bytes.len())))
        }
        b'<' if bytes.get(cursor + 1) != Some(&b'<') => {
            cursor += 1;
            while cursor < bytes.len() && bytes[cursor] != b'>' {
                cursor += 1;
            }
            Some((ContentToken::Other, (cursor + 1).min(bytes.len())))
        }
        byte if is_pdf_delimiter_or_whitespace(byte) => Some((ContentToken::Other, cursor + 1)),
        _ => {
            let start = cursor;
            while cursor < bytes.len() && !is_pdf_delimiter_or_whitespace(bytes[cursor]) {
                cursor += 1;
            }
            Some((ContentToken::Bare(bytes[start..cursor].to_vec()), cursor))
        }
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
    use std::collections::BTreeMap;

    use lopdf::{Dictionary, Document, Object, Stream, dictionary};

    use super::{
        ContentCache, content_without_inline_images, execute_content, is_pdf_boundary,
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
        assert_eq!(tokenized.ordinary_content, b" \n1 2 MaiUnknown\n");
    }

    #[test]
    fn ordinary_operation_parsing_resumes_after_each_inline_image() {
        assert_eq!(
            content_without_inline_images(b"q BI /W 1 /H 1 ID x EI Q BI /W 1 /H 1 ID y EI /Im Do"),
            b"q  Q  /Im Do"
        );
    }

    #[test]
    fn inline_image_ei_accepts_delimiter_boundaries() {
        assert_eq!(
            content_without_inline_images(b"BI /W 1 /H 1 ID x EI/Q"),
            b" /Q"
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
                .iter()
                .filter(|use_| use_.key.object_id() == Some(form_id.into()))
                .count(),
            4,
            "two ordinary invocations and their two cycle-edge observations remain visible"
        );
    }

    #[test]
    fn aggregate_content_decode_limit_is_preserved() {
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
            &BTreeMap::from([(1, PageEntry::Indirect(page_id))]),
            &mut ContentCache::new(),
            &limits,
        )
        .expect_err("decode limit");
        assert!(matches!(error, crate::PdfError::ContentDecodeLimit(16)));
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
            &BTreeMap::from([(1, PageEntry::Indirect(page_id))]),
            &mut ContentCache::new(),
            &SafetyLimits::default(),
        )
    }
}
