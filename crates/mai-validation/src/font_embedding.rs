use std::collections::{BTreeMap, BTreeSet};

use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::PdfObjectId;

#[derive(Clone, Debug, Default)]
pub(crate) struct FontEmbeddingSummary {
    pub(crate) failures: Vec<FontEmbeddingFailure>,
}

#[derive(Clone, Debug)]
pub(crate) struct FontEmbeddingFailure {
    pub(crate) object_id: Option<PdfObjectId>,
    pub(crate) description: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum FontKey {
    Indirect(ObjectId),
    Direct(String),
}

#[derive(Clone)]
struct SelectedFont {
    key: FontKey,
    object: Object,
    description: String,
}

#[derive(Clone, Default)]
struct GraphicsState {
    font: Option<SelectedFont>,
    rendering_mode: i64,
}

#[derive(Default)]
struct FontUse {
    object_id: Option<PdfObjectId>,
    description: String,
    subtype: Option<String>,
    embedded: bool,
    visible: bool,
}

struct Scanner<'a> {
    document: &'a Document,
    limits: &'a SafetyLimits,
    uses: BTreeMap<FontKey, FontUse>,
    active_descendant_fonts: BTreeSet<FontKey>,
}

pub(crate) fn inspect(
    document: &Document,
    limits: &SafetyLimits,
) -> Result<FontEmbeddingSummary, PdfError> {
    let mut scanner = Scanner {
        document,
        limits,
        uses: BTreeMap::new(),
        active_descendant_fonts: BTreeSet::new(),
    };
    for (page_number, page_id) in document.get_pages() {
        scanner.scan_page(page_number, page_id)?;
    }

    let failures = scanner
        .uses
        .into_values()
        .filter(|usage| {
            usage.visible
                && !usage.embedded
                && !matches!(usage.subtype.as_deref(), Some("Type3" | "Type0"))
        })
        .map(|usage| FontEmbeddingFailure {
            object_id: usage.object_id,
            description: usage.description,
        })
        .collect();
    Ok(FontEmbeddingSummary { failures })
}

impl Scanner<'_> {
    fn scan_page(&mut self, page_number: u32, page_id: ObjectId) -> Result<(), PdfError> {
        let page = self
            .document
            .objects
            .get(&page_id)
            .and_then(|object| object.as_dict().ok())
            .ok_or(PdfError::UnexpectedObject("page is not a dictionary"))?;
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
        let bytes = self.decode_stream(stream, decoded_bytes)?;
        let Ok(content) = Content::decode(&bytes) else {
            return Ok(());
        };
        for operation in content.operations {
            match operation.operator.as_str() {
                "q" => {
                    if stack.len() >= self.limits.max_reference_depth {
                        return Err(PdfError::ReferenceDepth(self.limits.max_reference_depth));
                    }
                    stack.push(state.clone());
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
                        Some(name) => resource(
                            self.document,
                            self.limits,
                            resources,
                            b"Font",
                            name,
                        )?
                        .map(|object| SelectedFont {
                            key: object_key(object, context, operation.operands.first()),
                            object: object.clone(),
                            description: describe_font(object, context, operation.operands.first()),
                        }),
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
                        self.record_font(&font, state.rendering_mode)?;
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
                        resource(self.document, self.limits, resources, b"XObject", name)?
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
        if form
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|value| value.as_name().ok())
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

    fn record_font(
        &mut self,
        selected: &SelectedFont,
        rendering_mode: i64,
    ) -> Result<(), PdfError> {
        if self.uses.contains_key(&selected.key) {
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
        let subtype = font
            .get(b"Subtype")
            .ok()
            .and_then(|value| value.as_name().ok())
            .map(|value| String::from_utf8_lossy(value).into_owned());
        let embedded =
            if rendering_mode == 3 || matches!(subtype.as_deref(), Some("Type3" | "Type0")) {
                false
            } else {
                font_is_embedded(self.document, font, self.limits)?
            };
        self.uses.entry(selected.key.clone()).or_insert(FontUse {
            object_id: match selected.key {
                FontKey::Indirect(id) => Some(id.into()),
                FontKey::Direct(_) => None,
            },
            description: selected.description.clone(),
            subtype: subtype.clone(),
            embedded,
            // veraPDF 1.28.2 associates the first observed rendering mode
            // with a font model object and does not revise it on later uses.
            visible: rendering_mode != 3,
        });

        if subtype.as_deref() == Some("Type0")
            && self.active_descendant_fonts.insert(selected.key.clone())
        {
            if let Ok(descendants) = font.get(b"DescendantFonts")
                && let Some(descendants) =
                    resolve_optional(self.document, descendants, self.limits.max_reference_depth)?
                        .and_then(|object| object.as_array().ok())
            {
                for (index, descendant) in descendants.iter().enumerate() {
                    let context = format!("{}/descendant {index}", selected.description);
                    let descendant = SelectedFont {
                        key: object_key(descendant, &context, None),
                        object: descendant.clone(),
                        description: describe_descendant(descendant, &selected.description, index),
                    };
                    self.record_font(&descendant, rendering_mode)?;
                }
            }
            self.active_descendant_fonts.remove(&selected.key);
        }
        Ok(())
    }

    fn decode_stream(
        &self,
        stream: &Stream,
        decoded_bytes: &mut usize,
    ) -> Result<Vec<u8>, PdfError> {
        let remaining = self
            .limits
            .max_decoded_stream_size
            .saturating_sub(*decoded_bytes);
        let bytes = match stream.decompressed_content_with_limit(remaining) {
            Ok(bytes) => bytes,
            Err(lopdf::Error::Decompress(lopdf::DecompressError::MemoryLimitExceeded {
                ..
            })) => {
                return Err(PdfError::ContentDecodeLimit(
                    self.limits.max_decoded_stream_size,
                ));
            }
            Err(_) if stream.content.len() <= remaining => stream.content.clone(),
            Err(_) => {
                return Err(PdfError::ContentDecodeLimit(
                    self.limits.max_decoded_stream_size,
                ));
            }
        };
        if bytes.len() > remaining {
            return Err(PdfError::ContentDecodeLimit(
                self.limits.max_decoded_stream_size,
            ));
        }
        *decoded_bytes = decoded_bytes.saturating_add(bytes.len());
        Ok(bytes)
    }
}

fn inherited_page_resources<'a>(
    document: &'a Document,
    mut node: &'a Dictionary,
    limits: &SafetyLimits,
) -> Result<Option<&'a Dictionary>, PdfError> {
    let mut visited = BTreeSet::new();
    for _ in 0..=limits.max_reference_depth {
        if let Ok(resources) = node.get(b"Resources") {
            return Ok(
                resolve_optional(document, resources, limits.max_reference_depth)?
                    .and_then(|object| object.as_dict().ok()),
            );
        }
        let Ok(parent) = node.get(b"Parent") else {
            return Ok(None);
        };
        if let Object::Reference(id) = parent
            && !visited.insert(*id)
        {
            return Err(PdfError::ReferenceDepth(limits.max_reference_depth));
        }
        let Some(parent) = resolve_optional(document, parent, limits.max_reference_depth)? else {
            return Ok(None);
        };
        let Ok(parent) = parent.as_dict() else {
            return Ok(None);
        };
        node = parent;
    }
    Err(PdfError::ReferenceDepth(limits.max_reference_depth))
}

fn resource<'a>(
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

fn resolve<'a>(
    document: &'a Document,
    mut object: &'a Object,
    maximum_depth: usize,
) -> Result<&'a Object, PdfError> {
    let mut visited = BTreeSet::new();
    for _ in 0..=maximum_depth {
        let Object::Reference(id) = object else {
            return Ok(object);
        };
        if !visited.insert(*id) {
            return Err(PdfError::ReferenceDepth(maximum_depth));
        }
        object = document
            .objects
            .get(id)
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

fn font_is_embedded(
    document: &Document,
    font: &Dictionary,
    limits: &SafetyLimits,
) -> Result<bool, PdfError> {
    let Ok(descriptor) = font.get(b"FontDescriptor") else {
        return Ok(false);
    };
    let Some(descriptor) = resolve_optional(document, descriptor, limits.max_reference_depth)?
        .and_then(|object| object.as_dict().ok())
    else {
        return Ok(false);
    };
    // `containsFontFile` is a veraPDF model property, not mere key presence:
    // the pinned malformed-program case proves that the stream must be
    // recognized as an embedded font program.
    for key in [b"FontFile".as_slice(), b"FontFile2", b"FontFile3"] {
        if let Ok(file) = descriptor.get(key)
            && let Some(stream) = resolve_optional(document, file, limits.max_reference_depth)?
                .and_then(|object| object.as_stream().ok())
            && valid_font_program(key, stream, limits)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn valid_font_program(
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
        b"FontFile" => bytes.starts_with(b"%!PS-AdobeFont") || bytes.starts_with(&[0x80, 0x01]),
        b"FontFile2" => valid_sfnt(&bytes),
        b"FontFile3" => {
            matches!(
                stream
                    .dict
                    .get(b"Subtype")
                    .ok()
                    .and_then(|value| value.as_name().ok()),
                Some(b"Type1C" | b"CIDFontType0C")
            ) && bytes.len() >= 4
                && matches!(bytes[0], 1 | 2)
                && usize::from(bytes[2]) <= bytes.len()
        }
        _ => false,
    })
}

fn valid_sfnt(bytes: &[u8]) -> bool {
    if bytes.len() < 12 || !matches!(&bytes[..4], b"\0\x01\0\0" | b"true" | b"typ1" | b"OTTO") {
        return false;
    }
    let table_count = usize::from(u16::from_be_bytes([bytes[4], bytes[5]]));
    let Some(directory_end) = table_count
        .checked_mul(16)
        .and_then(|length| 12usize.checked_add(length))
    else {
        return false;
    };
    if directory_end > bytes.len() {
        return false;
    }
    let mut tags = BTreeSet::new();
    for record in bytes[12..directory_end].chunks_exact(16) {
        let offset =
            u32::from_be_bytes(record[8..12].try_into().expect("four-byte table offset")) as usize;
        let length =
            u32::from_be_bytes(record[12..16].try_into().expect("four-byte table length")) as usize;
        if offset
            .checked_add(length)
            .is_none_or(|end| end > bytes.len())
        {
            return false;
        }
        tags.insert(&record[..4]);
    }
    [b"cmap", b"head", b"hhea", b"hmtx", b"maxp", b"name"]
        .iter()
        .all(|tag| tags.contains(tag.as_slice()))
}

fn object_key(object: &Object, context: &str, name: Option<&Object>) -> FontKey {
    match object {
        Object::Reference(id) => FontKey::Indirect(*id),
        _ => FontKey::Direct(format!(
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

fn describe_descendant(object: &Object, parent: &str, index: usize) -> String {
    match object {
        Object::Reference((number, generation)) => {
            format!("descendant font object {number} {generation}")
        }
        _ => format!("direct descendant font {index} of {parent}"),
    }
}

fn shows_text(operands: &[Object]) -> bool {
    operands.iter().any(|operand| match operand {
        Object::String(bytes, _) => !bytes.is_empty(),
        Object::Array(items) => shows_text(items),
        _ => false,
    })
}
