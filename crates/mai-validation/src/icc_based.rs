use std::collections::{BTreeMap, BTreeSet};

use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::model::{IccHeader, PdfObjectId};

#[derive(Clone, Debug, Default)]
pub(crate) struct IccBasedSummary {
    pub(crate) failures: Vec<IccBasedFailure>,
}

#[derive(Clone, Debug)]
pub(crate) struct IccBasedFailure {
    pub(crate) object_id: Option<PdfObjectId>,
    pub(crate) description: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ProfileKey {
    Indirect(ObjectId),
    Direct(String),
}

struct Scanner<'a> {
    document: &'a Document,
    limits: &'a SafetyLimits,
    inspected_profiles: BTreeSet<ProfileKey>,
    failures: BTreeMap<ProfileKey, IccBasedFailure>,
}

pub(crate) fn inspect(
    document: &Document,
    limits: &SafetyLimits,
) -> Result<IccBasedSummary, PdfError> {
    let mut scanner = Scanner {
        document,
        limits,
        inspected_profiles: BTreeSet::new(),
        failures: BTreeMap::new(),
    };
    for (page_number, page_id) in document.get_pages() {
        scanner.scan_page(page_number, page_id)?;
    }
    Ok(IccBasedSummary {
        failures: scanner.failures.into_values().collect(),
    })
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
        let mut active_xobjects = BTreeSet::new();
        let mut decoded_bytes = 0usize;
        if let Ok(contents) = page.get(b"Contents") {
            self.scan_contents(
                contents,
                resources,
                resources,
                &mut active_xobjects,
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
        page_resources: Option<&Dictionary>,
        active_xobjects: &mut BTreeSet<ObjectId>,
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
                    page_resources,
                    active_xobjects,
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
        let bytes = self.decode_content_stream(stream, decoded_bytes)?;
        for name in inline_image_color_space_names(&bytes) {
            self.inspect_selected_color_space(
                &Object::Name(name),
                resources,
                page_resources,
                &format!("{context}/inline image"),
            )?;
        }
        let Ok(content) = Content::decode(&bytes) else {
            return Ok(());
        };
        for operation in content.operations {
            match operation.operator.as_str() {
                "CS" | "cs" => {
                    if let Some(color_space) = operation.operands.first() {
                        self.inspect_selected_color_space(
                            color_space,
                            resources,
                            page_resources,
                            &format!("{context}/{}", operation.operator),
                        )?;
                    }
                }
                "g" | "G" => {
                    self.inspect_default_color_space(
                        b"DefaultGray",
                        resources,
                        page_resources,
                        context,
                    )?;
                }
                "rg" | "RG" => {
                    self.inspect_default_color_space(
                        b"DefaultRGB",
                        resources,
                        page_resources,
                        context,
                    )?;
                }
                "k" | "K" => {
                    self.inspect_default_color_space(
                        b"DefaultCMYK",
                        resources,
                        page_resources,
                        context,
                    )?;
                }
                "Do" => {
                    let Some(name) = operation
                        .operands
                        .first()
                        .and_then(|operand| operand.as_name().ok())
                    else {
                        continue;
                    };
                    let Some(xobject) = resource(
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
                    self.inspect_xobject(
                        xobject,
                        resources,
                        page_resources,
                        active_xobjects,
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
                    if let Ok(color_space) = shading.get(b"ColorSpace") {
                        self.inspect_selected_color_space(
                            color_space,
                            resources,
                            page_resources,
                            &format!("{context}/Shading /{}", String::from_utf8_lossy(name)),
                        )?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn inspect_xobject(
        &mut self,
        object: &Object,
        resources: Option<&Dictionary>,
        page_resources: Option<&Dictionary>,
        active_xobjects: &mut BTreeSet<ObjectId>,
        decoded_bytes: &mut usize,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        let object_id = object.as_reference().ok();
        if depth > self.limits.max_reference_depth {
            return Err(PdfError::ReferenceDepth(self.limits.max_reference_depth));
        }
        if object_id.is_some_and(|id| !active_xobjects.insert(id)) {
            return Ok(());
        }
        let result = self.inspect_xobject_inner(
            object,
            resources,
            page_resources,
            active_xobjects,
            decoded_bytes,
            context,
            depth,
        );
        if let Some(id) = object_id {
            active_xobjects.remove(&id);
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn inspect_xobject_inner(
        &mut self,
        object: &Object,
        resources: Option<&Dictionary>,
        page_resources: Option<&Dictionary>,
        active_xobjects: &mut BTreeSet<ObjectId>,
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
        match stream
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|value| value.as_name().ok())
        {
            Some(b"Image") => {
                let image_mask = stream
                    .dict
                    .get(b"ImageMask")
                    .ok()
                    .and_then(|value| value.as_bool().ok())
                    == Some(true);
                if !image_mask && let Ok(color_space) = stream.dict.get(b"ColorSpace") {
                    self.inspect_selected_color_space(
                        color_space,
                        resources,
                        page_resources,
                        context,
                    )?;
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
                            self.inspect_xobject(
                                image,
                                resources,
                                page_resources,
                                active_xobjects,
                                decoded_bytes,
                                &format!("{context}/Alternate {index}"),
                                depth + 1,
                            )?;
                        }
                    }
                }
            }
            Some(b"Form") => {
                let form_resources = match stream.dict.get(b"Resources") {
                    Ok(entry) => {
                        resolve_optional(self.document, entry, self.limits.max_reference_depth)?
                            .and_then(|object| object.as_dict().ok())
                    }
                    Err(_) => None,
                };
                self.scan_contents(
                    object,
                    form_resources,
                    page_resources,
                    active_xobjects,
                    decoded_bytes,
                    context,
                    depth,
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    fn inspect_selected_color_space(
        &mut self,
        value: &Object,
        resources: Option<&Dictionary>,
        page_resources: Option<&Dictionary>,
        context: &str,
    ) -> Result<(), PdfError> {
        if let Ok(name) = value.as_name()
            && let Some(color_space) = resource(
                self.document,
                self.limits,
                resources,
                page_resources,
                b"ColorSpace",
                name,
            )?
        {
            return self.inspect_color_space(
                color_space,
                &format!("{context} /{}", String::from_utf8_lossy(name)),
            );
        }
        self.inspect_color_space(value, context)
    }

    fn inspect_default_color_space(
        &mut self,
        name: &[u8],
        resources: Option<&Dictionary>,
        page_resources: Option<&Dictionary>,
        context: &str,
    ) -> Result<(), PdfError> {
        if let Some(color_space) = resource(
            self.document,
            self.limits,
            resources,
            page_resources,
            b"ColorSpace",
            name,
        )? {
            self.inspect_color_space(
                color_space,
                &format!("{context}/{}", String::from_utf8_lossy(name)),
            )?;
        }
        Ok(())
    }

    fn inspect_color_space(&mut self, value: &Object, context: &str) -> Result<(), PdfError> {
        self.inspect_color_space_at_depth(value, context, 0)
    }

    fn inspect_color_space_at_depth(
        &mut self,
        value: &Object,
        context: &str,
        depth: usize,
    ) -> Result<(), PdfError> {
        if depth > self.limits.max_reference_depth {
            return Err(PdfError::ReferenceDepth(self.limits.max_reference_depth));
        }
        let Some(value) = resolve_optional(self.document, value, self.limits.max_reference_depth)?
        else {
            return Ok(());
        };
        let Ok(items) = value.as_array() else {
            return Ok(());
        };
        match items.first().and_then(|item| item.as_name().ok()) {
            Some(b"Indexed") => {
                if let Some(base) = items.get(1) {
                    self.inspect_color_space_at_depth(
                        base,
                        &format!("{context}/Indexed base"),
                        depth + 1,
                    )?;
                }
                return Ok(());
            }
            Some(b"ICCBased") => {}
            _ => return Ok(()),
        }
        let Some(profile) = items.get(1) else {
            return Ok(());
        };
        let key = match profile {
            Object::Reference(id) => ProfileKey::Indirect(*id),
            _ => ProfileKey::Direct(context.to_owned()),
        };
        if !self.inspected_profiles.insert(key.clone()) {
            return Ok(());
        }
        let Some(stream) =
            resolve_optional(self.document, profile, self.limits.max_reference_depth)?
                .and_then(|object| object.as_stream().ok())
        else {
            return Ok(());
        };
        let bytes =
            match stream.decompressed_content_with_limit(self.limits.max_decoded_stream_size) {
                Ok(bytes) => bytes,
                Err(
                    error @ lopdf::Error::Decompress(lopdf::DecompressError::MemoryLimitExceeded {
                        ..
                    }),
                ) => {
                    return Err(PdfError::IccDecodeLimit(format!(
                        "ICCBased profile: {error}"
                    )));
                }
                Err(error) => {
                    self.record_failure(
                        key,
                        format!("could not decode ICCBased profile stream: {error}"),
                    );
                    return Ok(());
                }
            };
        let Some(header) = IccHeader::parse(&bytes) else {
            self.record_failure(
                key,
                "ICCBased profile is shorter than the 20-byte header prefix required by this check"
                    .to_owned(),
            );
            return Ok(());
        };
        if !header.conforms_to_pdfa_1_input_profile() {
            self.record_failure(
                key,
                format!(
                    "ICCBased profile has class {:?}, colour space {:?}, and version {}.{}",
                    header.device_class,
                    header.color_space,
                    header.version_major,
                    header.version_minor
                ),
            );
        }
        Ok(())
    }

    fn record_failure(&mut self, key: ProfileKey, description: String) {
        let object_id = match key {
            ProfileKey::Indirect(id) => Some(id.into()),
            ProfileKey::Direct(_) => None,
        };
        self.failures.entry(key).or_insert(IccBasedFailure {
            object_id,
            description,
        });
    }

    fn decode_content_stream(
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

#[derive(Clone, Debug, PartialEq, Eq)]
enum ContentToken {
    Name(Vec<u8>),
    Bare(Vec<u8>),
    OpenArray,
    CloseArray,
    Other,
}

fn inline_image_color_space_names(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut names = Vec::new();
    let mut cursor = 0usize;
    let mut array_depth = 0usize;
    while let Some((token, next)) = next_content_token(bytes, cursor) {
        cursor = next;
        match token {
            ContentToken::OpenArray => array_depth = array_depth.saturating_add(1),
            ContentToken::CloseArray => array_depth = array_depth.saturating_sub(1),
            ContentToken::Bare(token) if array_depth == 0 && token == b"BI" => {
                let mut color_space_key = false;
                while let Some((token, next)) = next_content_token(bytes, cursor) {
                    cursor = next;
                    match token {
                        ContentToken::Bare(token) if token == b"ID" => {
                            cursor = find_inline_image_end(bytes, cursor).unwrap_or(bytes.len());
                            break;
                        }
                        ContentToken::Name(name) if color_space_key => {
                            names.push(name);
                            color_space_key = false;
                        }
                        ContentToken::Name(name) => {
                            color_space_key = matches!(name.as_slice(), b"CS" | b"ColorSpace");
                        }
                        _ => color_space_key = false,
                    }
                }
            }
            _ => {}
        }
    }
    names
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
    bytes
        .get(cursor..)?
        .windows(4)
        .position(|window| {
            is_pdf_whitespace(window[0])
                && window[1] == b'E'
                && window[2] == b'I'
                && is_pdf_whitespace(window[3])
        })
        .map(|offset| cursor + offset + 3)
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
    page_resources: Option<&'a Dictionary>,
    category: &[u8],
    name: &[u8],
) -> Result<Option<&'a Object>, PdfError> {
    if let Some(object) = resource_once(document, limits, resources, category, name)? {
        return Ok(Some(object));
    }
    resource_once(document, limits, page_resources, category, name)
}

fn resource_once<'a>(
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

#[cfg(test)]
mod tests {
    use super::inline_image_color_space_names;

    #[test]
    fn extracts_inline_image_resource_names_without_reading_strings_as_operators() {
        let bytes = b"(BI /CS /Ignored ID x EI) Tj\n\
            BI /W 1 /H 1 /BPC 8 /CS /First ID \0\0\0 EI\n\
            BI /W 1 /H 1 /BPC 8 /ColorSpace /Second ID \0\0\0 EI\n";
        assert_eq!(
            inline_image_color_space_names(bytes),
            vec![b"First".to_vec(), b"Second".to_vec()]
        );
    }

    #[test]
    fn decodes_hex_escapes_in_inline_image_resource_names() {
        assert_eq!(
            inline_image_color_space_names(b"BI /CS /C#53#31 /W 1 /H 1 /BPC 8 ID x EI\n"),
            vec![b"CS1".to_vec()]
        );
    }
}
