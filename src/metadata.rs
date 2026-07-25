use std::collections::BTreeMap;

use roxmltree::{Document, ParsingOptions};
use serde::Serialize;

const PDFA_ID_NAMESPACE: &str = "http://www.aiim.org/pdfa/ns/id/";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct DocumentMetadata {
    pub values: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct XmpMetadata {
    pub pdfa_part: Option<String>,
    pub pdfa_conformance: Option<String>,
    pub byte_length: usize,
}

pub(crate) fn parse_xmp(bytes: &[u8]) -> Result<XmpMetadata, String> {
    let xml = decode_xml(bytes)?;
    let options = ParsingOptions {
        allow_dtd: false,
        nodes_limit: 100_000,
        ..ParsingOptions::default()
    };
    let document =
        Document::parse_with_options(&xml, options).map_err(|error| error.to_string())?;

    let part = find_pdfa_property(&document, "part");
    let conformance = find_pdfa_property(&document, "conformance");

    Ok(XmpMetadata {
        pdfa_part: part,
        pdfa_conformance: conformance,
        byte_length: bytes.len(),
    })
}

fn find_pdfa_property(document: &Document<'_>, local_name: &str) -> Option<String> {
    for node in document.descendants().filter(|node| node.is_element()) {
        if node.tag_name().namespace() == Some(PDFA_ID_NAMESPACE)
            && node.tag_name().name() == local_name
            && let Some(value) = node.text()
        {
            return Some(value.trim().to_owned());
        }

        for attribute in node.attributes() {
            if attribute.namespace() == Some(PDFA_ID_NAMESPACE) && attribute.name() == local_name {
                return Some(attribute.value().trim().to_owned());
            }
        }
    }
    None
}

fn decode_xml(bytes: &[u8]) -> Result<String, String> {
    if let Some(bytes) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8(bytes.to_vec()).map_err(|error| error.to_string());
    }
    if let Some(bytes) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return decode_utf16(bytes, u16::from_be_bytes);
    }
    if let Some(bytes) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return decode_utf16(bytes, u16::from_le_bytes);
    }
    String::from_utf8(bytes.to_vec()).map_err(|error| error.to_string())
}

fn decode_utf16(bytes: &[u8], convert: fn([u8; 2]) -> u16) -> Result<String, String> {
    if !bytes.len().is_multiple_of(2) {
        return Err("UTF-16 XML has an odd byte length".to_owned());
    }
    let units = bytes
        .chunks_exact(2)
        .map(|pair| convert([pair[0], pair[1]]));
    std::char::decode_utf16(units)
        .collect::<Result<String, _>>()
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pdfa_identification_elements() {
        let xmp = br#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
          <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
            <rdf:Description xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/">
              <pdfaid:part>1</pdfaid:part>
              <pdfaid:conformance>B</pdfaid:conformance>
            </rdf:Description>
          </rdf:RDF>
        </x:xmpmeta>"#;
        let parsed = parse_xmp(xmp).expect("valid XMP");
        assert_eq!(parsed.pdfa_part.as_deref(), Some("1"));
        assert_eq!(parsed.pdfa_conformance.as_deref(), Some("B"));
    }

    #[test]
    fn parses_pdfa_identification_attributes() {
        let xmp = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/">
          <rdf:Description pdfaid:part="1" pdfaid:conformance="B"/>
        </rdf:RDF>"#;
        let parsed = parse_xmp(xmp).expect("valid XMP");
        assert_eq!(parsed.pdfa_part.as_deref(), Some("1"));
        assert_eq!(parsed.pdfa_conformance.as_deref(), Some("B"));
    }

    #[test]
    fn rejects_malformed_xmp() {
        assert!(parse_xmp(b"<rdf:RDF>").is_err());
    }
}
