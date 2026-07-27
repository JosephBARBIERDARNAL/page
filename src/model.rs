use std::collections::{BTreeMap, BTreeSet};

use lopdf::{Dictionary, Document, LoadOptions, Object, ObjectId};
use serde::Serialize;

use crate::error::PdfError;
use crate::limits::SafetyLimits;
use crate::metadata::{DocumentMetadata, XmpMetadata, parse_xmp};

type XmpExtraction = (Option<XmpMetadata>, Option<PdfObjectId>, Option<String>);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct PdfObjectId {
    pub object_number: u32,
    pub generation: u16,
}

impl From<ObjectId> for PdfObjectId {
    fn from((object_number, generation): ObjectId) -> Self {
        Self {
            object_number,
            generation,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct FontSummary {
    pub total: usize,
    pub embedded: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct CatalogMetadataStream {
    pub present: bool,
    pub is_stream: bool,
    pub type_is_metadata: bool,
    pub subtype_is_xml: bool,
    pub has_filter: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct PdfDocument {
    pub version: String,
    pub encrypted: bool,
    pub catalog_reference: Option<PdfObjectId>,
    pub catalog_present: bool,
    pub page_count: usize,
    pub trailer_keys: Vec<String>,
    pub info: DocumentMetadata,
    pub info_object: Option<PdfObjectId>,
    pub xmp: Option<XmpMetadata>,
    pub xmp_object: Option<PdfObjectId>,
    pub xmp_parse_error: Option<String>,
    pub catalog_metadata: CatalogMetadataStream,
    pub output_intents: Vec<Option<PdfObjectId>>,
    pub fonts: FontSummary,
    pub object_count: usize,
}

impl PdfDocument {
    pub fn from_bytes(bytes: &[u8], limits: &SafetyLimits) -> Result<Self, PdfError> {
        if bytes.len() as u64 > limits.max_input_size {
            return Err(PdfError::InputTooLarge {
                actual: bytes.len() as u64,
                limit: limits.max_input_size,
            });
        }

        let options = LoadOptions {
            strict: true,
            max_decompressed_size: Some(limits.max_decoded_stream_size),
            ..LoadOptions::default()
        };
        let document = Document::load_mem_with_options(bytes, options)?;
        if document.objects.len() > limits.max_object_count {
            return Err(PdfError::TooManyObjects {
                actual: document.objects.len(),
                limit: limits.max_object_count,
            });
        }

        Self::normalize(&document, limits)
    }

    fn normalize(document: &Document, limits: &SafetyLimits) -> Result<Self, PdfError> {
        let root = document.trailer.get(b"Root").ok();
        let catalog_reference = root
            .and_then(|object| object.as_reference().ok())
            .map(PdfObjectId::from);
        let encrypted = document.was_encrypted() || document.trailer.has(b"Encrypt");
        let mut trailer_keys = document
            .trailer
            .iter()
            .map(|(key, _)| String::from_utf8_lossy(key).into_owned())
            .collect::<Vec<_>>();
        trailer_keys.sort();

        // A password-protected document may expose its trailer while keeping
        // referenced objects inaccessible. Encryption alone is enough to fail
        // PDF/A-1b, so retain the available structure without treating those
        // inaccessible objects as a syntax error.
        if encrypted {
            return Ok(Self {
                version: document.version.clone(),
                encrypted: true,
                catalog_reference,
                catalog_present: false,
                page_count: 0,
                trailer_keys,
                info: DocumentMetadata::default(),
                info_object: None,
                xmp: None,
                xmp_object: None,
                xmp_parse_error: None,
                catalog_metadata: CatalogMetadataStream::default(),
                output_intents: Vec::new(),
                fonts: FontSummary::default(),
                object_count: document.objects.len(),
            });
        }

        let catalog = match root.filter(|_| catalog_reference.is_some()) {
            Some(root) => match resolve(document, root, limits.max_reference_depth) {
                Ok(object) => object
                    .as_dict()
                    .ok()
                    .filter(|dictionary| dictionary.get_type().ok() == Some(b"Catalog".as_slice())),
                Err(error @ PdfError::ReferenceDepth(_)) => return Err(error),
                Err(_) => None,
            },
            None => None,
        };

        let (info, info_object) = extract_info(document, limits)?;
        let catalog_metadata = inspect_catalog_metadata(document, catalog, limits)?;
        let (xmp, xmp_object, xmp_parse_error) = extract_xmp(document, catalog, limits)?;
        let output_intents = extract_output_intents(document, catalog, limits)?;

        Ok(Self {
            version: document.version.clone(),
            encrypted: false,
            catalog_reference,
            catalog_present: catalog.is_some(),
            page_count: catalog
                .map(|catalog| count_pages(document, catalog, limits.max_reference_depth))
                .unwrap_or(0),
            trailer_keys,
            info,
            info_object,
            xmp,
            xmp_object,
            xmp_parse_error,
            catalog_metadata,
            output_intents,
            fonts: summarize_fonts(document, limits),
            object_count: document.objects.len(),
        })
    }
}

fn inspect_catalog_metadata(
    document: &Document,
    catalog: Option<&Dictionary>,
    limits: &SafetyLimits,
) -> Result<CatalogMetadataStream, PdfError> {
    let Some(catalog) = catalog else {
        return Ok(CatalogMetadataStream::default());
    };
    let Ok(entry) = catalog.get(b"Metadata") else {
        return Ok(CatalogMetadataStream::default());
    };
    let mut result = CatalogMetadataStream {
        present: true,
        ..CatalogMetadataStream::default()
    };
    let Ok(object) = resolve(document, entry, limits.max_reference_depth) else {
        return Ok(result);
    };
    let Ok(stream) = object.as_stream() else {
        return Ok(result);
    };
    result.is_stream = true;
    result.type_is_metadata = stream.dict.get_type().ok() == Some(b"Metadata".as_slice());
    result.subtype_is_xml = stream
        .dict
        .get(b"Subtype")
        .ok()
        .and_then(|object| object.as_name().ok())
        == Some(b"XML".as_slice());
    result.has_filter = stream.dict.has(b"Filter");
    Ok(result)
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

fn count_pages(document: &Document, catalog: &Dictionary, maximum_depth: usize) -> usize {
    let Ok(pages) = catalog.get(b"Pages") else {
        return 0;
    };
    let mut count = 0usize;
    let mut visited = BTreeSet::new();
    let mut stack = vec![(pages, 0usize)];

    while let Some((object, depth)) = stack.pop() {
        if depth > maximum_depth {
            continue;
        }
        if let Object::Reference(id) = object
            && !visited.insert(*id)
        {
            continue;
        }
        let Ok(object) = resolve(document, object, maximum_depth.saturating_sub(depth)) else {
            continue;
        };
        let Ok(dictionary) = object.as_dict() else {
            continue;
        };
        match dictionary.get_type().ok() {
            Some(b"Page") => count = count.saturating_add(1),
            Some(b"Pages") => {
                let Ok(kids) = dictionary.get(b"Kids") else {
                    continue;
                };
                let Ok(kids) = resolve(document, kids, maximum_depth.saturating_sub(depth))
                    .and_then(|object| {
                        object
                            .as_array()
                            .map_err(|_| PdfError::UnexpectedObject("Kids is not an array"))
                    })
                else {
                    continue;
                };
                stack.extend(kids.iter().rev().map(|kid| (kid, depth + 1)));
            }
            _ => {}
        }
    }
    count
}

fn extract_info(
    document: &Document,
    limits: &SafetyLimits,
) -> Result<(DocumentMetadata, Option<PdfObjectId>), PdfError> {
    let Ok(info_entry) = document.trailer.get(b"Info") else {
        return Ok((DocumentMetadata::default(), None));
    };
    let object_id = info_entry.as_reference().ok().map(Into::into);
    let info = resolve(document, info_entry, limits.max_reference_depth)?
        .as_dict()
        .map_err(|_| PdfError::UnexpectedObject("Info is not a dictionary"))?;

    let mut values = BTreeMap::new();
    for key in [
        b"Title".as_slice(),
        b"Author",
        b"Subject",
        b"Keywords",
        b"Creator",
        b"Producer",
        b"CreationDate",
        b"ModDate",
        b"Trapped",
    ] {
        if let Ok(value) = info.get(key)
            && let Some(text) = object_text(value)
        {
            values.insert(String::from_utf8_lossy(key).into_owned(), text);
        }
    }

    Ok((DocumentMetadata { values }, object_id))
}

fn object_text(object: &Object) -> Option<String> {
    match object {
        Object::String(bytes, _) => Some(decode_pdf_string(bytes)),
        Object::Name(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
        _ => None,
    }
}

fn decode_pdf_string(bytes: &[u8]) -> String {
    if let Some(bytes) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        let units = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]));
        String::from_utf16_lossy(&units.collect::<Vec<_>>())
    } else if let Some(bytes) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let units = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]));
        String::from_utf16_lossy(&units.collect::<Vec<_>>())
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

fn extract_xmp(
    document: &Document,
    catalog: Option<&Dictionary>,
    limits: &SafetyLimits,
) -> Result<XmpExtraction, PdfError> {
    let Some(catalog) = catalog else {
        return Ok((None, None, None));
    };
    let Ok(metadata_entry) = catalog.get(b"Metadata") else {
        return Ok((None, None, None));
    };
    let object_id = metadata_entry.as_reference().ok().map(Into::into);
    let stream =
        match resolve(document, metadata_entry, limits.max_reference_depth).and_then(|object| {
            object
                .as_stream()
                .map_err(|_| PdfError::UnexpectedObject("Metadata is not a stream"))
        }) {
            Ok(stream) => stream,
            Err(error) => return Ok((None, object_id, Some(error.to_string()))),
        };
    let bytes = match stream.decompressed_content_with_limit(limits.max_decoded_stream_size) {
        Ok(bytes) => bytes,
        Err(
            error @ lopdf::Error::Decompress(lopdf::DecompressError::MemoryLimitExceeded { .. }),
        ) => return Err(PdfError::XmpDecodeLimit(error.to_string())),
        Err(error) => {
            return Ok((
                None,
                object_id,
                Some(format!("could not decode XMP stream: {error}")),
            ));
        }
    };
    Ok(match parse_xmp(&bytes) {
        Ok(xmp) => (Some(xmp), object_id, None),
        Err(error) => (None, object_id, Some(error)),
    })
}

fn extract_output_intents(
    document: &Document,
    catalog: Option<&Dictionary>,
    limits: &SafetyLimits,
) -> Result<Vec<Option<PdfObjectId>>, PdfError> {
    let Some(catalog) = catalog else {
        return Ok(Vec::new());
    };
    let Ok(entry) = catalog.get(b"OutputIntents") else {
        return Ok(Vec::new());
    };
    let array = resolve(document, entry, limits.max_reference_depth)?
        .as_array()
        .map_err(|_| PdfError::UnexpectedObject("OutputIntents is not an array"))?;
    Ok(array
        .iter()
        .map(|item| item.as_reference().ok().map(Into::into))
        .collect())
}

fn summarize_fonts(document: &Document, limits: &SafetyLimits) -> FontSummary {
    let mut summary = FontSummary::default();
    for object in document.objects.values() {
        let dictionary = match object {
            Object::Dictionary(dictionary) => dictionary,
            Object::Stream(stream) => &stream.dict,
            _ => continue,
        };
        if dictionary.get_type().ok() != Some(b"Font".as_slice()) {
            continue;
        }
        summary.total += 1;
        if font_is_embedded(document, dictionary, limits) {
            summary.embedded += 1;
        }
    }
    summary
}

fn font_is_embedded(document: &Document, font: &Dictionary, limits: &SafetyLimits) -> bool {
    let Ok(descriptor_entry) = font.get(b"FontDescriptor") else {
        return false;
    };
    let Ok(descriptor) =
        resolve(document, descriptor_entry, limits.max_reference_depth).and_then(|object| {
            object
                .as_dict()
                .map_err(|_| PdfError::UnexpectedObject("FontDescriptor is not a dictionary"))
        })
    else {
        return false;
    };
    descriptor.has(b"FontFile") || descriptor.has(b"FontFile2") || descriptor.has(b"FontFile3")
}

#[cfg(test)]
mod tests {
    use lopdf::{Object, StringFormat, dictionary};

    use super::*;

    #[test]
    fn extracts_info_dictionary() {
        let mut document = Document::with_version("1.4");
        let info_id = document.add_object(dictionary! {
            "Title" => Object::String(b"Example".to_vec(), StringFormat::Literal),
            "Author" => Object::String(b"Ferris".to_vec(), StringFormat::Literal),
        });
        document.trailer.set("Info", info_id);
        let (metadata, id) = extract_info(&document, &SafetyLimits::default()).expect("metadata");
        assert_eq!(id, Some(info_id.into()));
        assert_eq!(metadata.values["Title"], "Example");
        assert_eq!(metadata.values["Author"], "Ferris");
    }
}
