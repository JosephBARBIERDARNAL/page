use std::collections::{BTreeMap, BTreeSet};

use roxmltree::{Attribute, Document, Node, ParsingOptions};
use serde::Serialize;

const RDF_NAMESPACE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
const PDFA_ID_NAMESPACE: &str = "http://www.aiim.org/pdfa/ns/id/";
const DC_NAMESPACE: &str = "http://purl.org/dc/elements/1.1/";
const PDF_NAMESPACE: &str = "http://ns.adobe.com/pdf/1.3/";
const XMP_NAMESPACE: &str = "http://ns.adobe.com/xap/1.0/";
const PDFA_EXTENSION_NAMESPACE: &str = "http://www.aiim.org/pdfa/ns/extension/";
const PDFA_SCHEMA_NAMESPACE: &str = "http://www.aiim.org/pdfa/ns/schema#";
const PDFA_PROPERTY_NAMESPACE: &str = "http://www.aiim.org/pdfa/ns/property#";
const PDFA_TYPE_NAMESPACE: &str = "http://www.aiim.org/pdfa/ns/type#";
const PDFA_FIELD_NAMESPACE: &str = "http://www.aiim.org/pdfa/ns/field#";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct DocumentMetadata {
    pub values: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct XmpMetadata {
    /// Retained for API/report compatibility. Validation uses `pdfa_parts`,
    /// because selecting only the first value would hide duplicate declarations.
    pub pdfa_part: Option<String>,
    pub pdfa_conformance: Option<String>,
    pub pdfa_identification_present: bool,
    pub pdfa_parts: Vec<String>,
    pub pdfa_conformances: Vec<String>,
    pub title_x_default: Vec<String>,
    pub creators: Vec<String>,
    pub creator_container_count: usize,
    pub description_x_default: Vec<String>,
    pub keywords: Vec<String>,
    pub creator_tools: Vec<String>,
    pub producers: Vec<String>,
    pub create_dates: Vec<String>,
    pub modify_dates: Vec<String>,
    pub byte_length: usize,
    #[serde(skip)]
    pub packet_header_has_bytes: bool,
    #[serde(skip)]
    pub packet_header_has_encoding: bool,
    #[serde(skip)]
    pub extension_schema_failed_tests: BTreeSet<u8>,
    #[serde(skip)]
    pub identification_prefix_failed_tests: BTreeSet<u8>,
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
    let packet_header = xmp_packet_header(&document);
    let extension_schema_failed_tests = inspect_extension_schemas(&document, &xml);
    let identification_prefix_failed_tests = inspect_identification_prefixes(&document, &xml);

    let pdfa_identification_present = contains_namespace_property(&document, PDFA_ID_NAMESPACE);
    let pdfa_parts = property_values(&document, PDFA_ID_NAMESPACE, "part");
    let pdfa_conformances = property_values(&document, PDFA_ID_NAMESPACE, "conformance");
    let title_x_default = alt_values(&document, DC_NAMESPACE, "title", "x-default");
    let creator_nodes = property_nodes(&document, DC_NAMESPACE, "creator");
    let creators = creator_nodes
        .iter()
        .flat_map(|node| container_values(*node, "Seq", None))
        .collect();
    let description_x_default = alt_values(&document, DC_NAMESPACE, "description", "x-default");

    Ok(XmpMetadata {
        pdfa_part: pdfa_parts.first().cloned(),
        pdfa_conformance: pdfa_conformances.first().cloned(),
        pdfa_identification_present,
        pdfa_parts,
        pdfa_conformances,
        title_x_default,
        creators,
        creator_container_count: creator_nodes.len(),
        description_x_default,
        keywords: property_values(&document, PDF_NAMESPACE, "Keywords"),
        creator_tools: property_values(&document, XMP_NAMESPACE, "CreatorTool"),
        producers: property_values(&document, PDF_NAMESPACE, "Producer"),
        create_dates: property_values(&document, XMP_NAMESPACE, "CreateDate"),
        modify_dates: property_values(&document, XMP_NAMESPACE, "ModifyDate"),
        byte_length: bytes.len(),
        packet_header_has_bytes: packet_header
            .is_some_and(|header| has_quoted_assignment(header, b"bytes")),
        packet_header_has_encoding: packet_header
            .is_some_and(|header| has_quoted_assignment(header, b"encoding")),
        extension_schema_failed_tests,
        identification_prefix_failed_tests,
    })
}

#[derive(Clone, Copy)]
enum XmpProperty<'a> {
    Element(Node<'a, 'a>),
    Attribute(Attribute<'a, 'a>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ArrayKind {
    Bag,
    Seq,
    Alt,
}

impl<'a> XmpProperty<'a> {
    fn namespace(self) -> Option<&'a str> {
        match self {
            Self::Element(node) => node.tag_name().namespace(),
            Self::Attribute(attribute) => attribute.namespace(),
        }
    }

    fn name(self) -> &'a str {
        match self {
            Self::Element(node) => node.tag_name().name(),
            Self::Attribute(attribute) => attribute.name(),
        }
    }

    fn prefix(self, xml: &str) -> Option<&str> {
        let qualified_name = match self {
            Self::Element(node) => {
                let range = node.range();
                let source = xml.get(range)?;
                source
                    .strip_prefix('<')?
                    .split(|character: char| {
                        character.is_ascii_whitespace() || matches!(character, '/' | '>')
                    })
                    .next()?
            }
            Self::Attribute(attribute) => xml.get(attribute.range_qname())?,
        };
        qualified_name.split_once(':').map(|(prefix, _)| prefix)
    }

    fn is_simple(self) -> bool {
        match self {
            Self::Attribute(_) => true,
            Self::Element(node) => {
                node.attribute((RDF_NAMESPACE, "parseType")) != Some("Resource")
                    && !node.children().any(|child| child.is_element())
            }
        }
    }

    fn value(self) -> Option<&'a str> {
        match self {
            Self::Attribute(attribute) => Some(attribute.value()),
            Self::Element(node) => node.text(),
        }
    }

    fn array_kind(self) -> Option<ArrayKind> {
        let Self::Element(node) = self else {
            return None;
        };
        node.children().find_map(|child| {
            if !child.is_element() || child.tag_name().namespace() != Some(RDF_NAMESPACE) {
                return None;
            }
            match child.tag_name().name() {
                "Bag" => Some(ArrayKind::Bag),
                "Seq" => Some(ArrayKind::Seq),
                "Alt" => Some(ArrayKind::Alt),
                _ => None,
            }
        })
    }

    fn array_items(self) -> Vec<Node<'a, 'a>> {
        let Self::Element(node) = self else {
            return Vec::new();
        };
        node.children()
            .find(|child| {
                child.is_element()
                    && child.tag_name().namespace() == Some(RDF_NAMESPACE)
                    && matches!(child.tag_name().name(), "Bag" | "Seq" | "Alt")
            })
            .into_iter()
            .flat_map(|container| container.children())
            .filter(|child| {
                child.is_element()
                    && child.tag_name().namespace() == Some(RDF_NAMESPACE)
                    && child.tag_name().name() == "li"
            })
            .collect()
    }
}

fn inspect_extension_schemas(document: &Document<'_>, xml: &str) -> BTreeSet<u8> {
    let mut failed = BTreeSet::new();
    for container in document.descendants().filter(|node| {
        node.is_element()
            && node.tag_name().namespace() == Some(PDFA_EXTENSION_NAMESPACE)
            && node.tag_name().name() == "schemas"
    }) {
        let container = XmpProperty::Element(container);
        if container.array_kind() != Some(ArrayKind::Bag)
            || container.prefix(xml) != Some("pdfaExtension")
        {
            failed.insert(2);
        }

        for definition in container.array_items() {
            inspect_schema_definition(definition, xml, &mut failed);
        }
    }
    failed
}

fn inspect_identification_prefixes(document: &Document<'_>, xml: &str) -> BTreeSet<u8> {
    [("part", 4), ("conformance", 5), ("amd", 6)]
        .into_iter()
        .filter_map(|(name, test)| {
            first_document_property(document, PDFA_ID_NAMESPACE, name)
                .and_then(|property| property.prefix(xml))
                .is_some_and(|prefix| prefix != "pdfaid")
                .then_some(test)
        })
        .collect()
}

fn first_document_property<'a>(
    document: &'a Document<'a>,
    namespace: &str,
    name: &str,
) -> Option<XmpProperty<'a>> {
    for node in document.descendants().filter(|node| node.is_element()) {
        if node.tag_name().namespace() == Some(namespace) && node.tag_name().name() == name {
            return Some(XmpProperty::Element(node));
        }
        if let Some(attribute) = node
            .attributes()
            .find(|attribute| attribute.namespace() == Some(namespace) && attribute.name() == name)
        {
            return Some(XmpProperty::Attribute(attribute));
        }
    }
    None
}

fn inspect_schema_definition(definition: Node<'_, '_>, xml: &str, failed: &mut BTreeSet<u8>) {
    if has_undefined_fields(
        definition,
        PDFA_SCHEMA_NAMESPACE,
        &["schema", "namespaceURI", "prefix", "property", "valueType"],
    ) {
        failed.insert(1);
    }
    for (test, name) in [(3, "schema"), (4, "namespaceURI"), (5, "prefix")] {
        if !required_simple_field_has_prefix(
            definition,
            PDFA_SCHEMA_NAMESPACE,
            name,
            "pdfaSchema",
            xml,
        ) {
            failed.insert(test);
        }
    }

    let known_types = known_types(definition);
    let property = first_field(definition, PDFA_SCHEMA_NAMESPACE, "property");
    if !optional_sequence_is_valid(property, "pdfaSchema", xml, property_item_is_valid) {
        failed.insert(6);
    }
    let value_type = first_field(definition, PDFA_SCHEMA_NAMESPACE, "valueType");
    if !optional_sequence_is_valid(value_type, "pdfaSchema", xml, |item| {
        value_type_item_is_valid(item)
    }) {
        failed.insert(7);
    }

    for property in fields(definition, PDFA_SCHEMA_NAMESPACE, "property") {
        if property.array_kind().is_some() {
            for item in property.array_items() {
                inspect_schema_property(item, xml, &known_types, failed);
            }
        }
    }
    for value_type in fields(definition, PDFA_SCHEMA_NAMESPACE, "valueType") {
        if value_type.array_kind().is_some() {
            for item in value_type.array_items() {
                inspect_schema_value_type(item, xml, &known_types, failed);
            }
        }
    }
}

fn inspect_schema_property(
    property: Node<'_, '_>,
    xml: &str,
    known_types: &BTreeSet<String>,
    failed: &mut BTreeSet<u8>,
) {
    if has_undefined_fields(
        property,
        PDFA_PROPERTY_NAMESPACE,
        &["name", "valueType", "category", "description"],
    ) {
        failed.insert(1);
    }
    if !required_simple_field_has_prefix(
        property,
        PDFA_PROPERTY_NAMESPACE,
        "name",
        "pdfaProperty",
        xml,
    ) {
        failed.insert(8);
    }
    if !required_simple_field_has_prefix(
        property,
        PDFA_PROPERTY_NAMESPACE,
        "valueType",
        "pdfaProperty",
        xml,
    ) || !field_value(property, PDFA_PROPERTY_NAMESPACE, "valueType")
        .is_some_and(|value| known_types.contains(value.trim()))
    {
        failed.insert(9);
    }
    if !required_simple_field_has_prefix(
        property,
        PDFA_PROPERTY_NAMESPACE,
        "category",
        "pdfaProperty",
        xml,
    ) || !matches!(
        field_value(property, PDFA_PROPERTY_NAMESPACE, "category"),
        Some("external" | "internal")
    ) {
        failed.insert(10);
    }
    if !required_simple_field_has_prefix(
        property,
        PDFA_PROPERTY_NAMESPACE,
        "description",
        "pdfaProperty",
        xml,
    ) {
        failed.insert(11);
    }
}

fn inspect_schema_value_type(
    value_type: Node<'_, '_>,
    xml: &str,
    known_types: &BTreeSet<String>,
    failed: &mut BTreeSet<u8>,
) {
    if has_undefined_fields(
        value_type,
        PDFA_TYPE_NAMESPACE,
        &["type", "namespaceURI", "prefix", "description", "field"],
    ) {
        failed.insert(1);
    }
    for (test, name) in [
        (12, "type"),
        (13, "namespaceURI"),
        (14, "prefix"),
        (15, "description"),
    ] {
        if !required_simple_field_has_prefix(value_type, PDFA_TYPE_NAMESPACE, name, "pdfaType", xml)
        {
            failed.insert(test);
        }
    }
    let field = first_field(value_type, PDFA_TYPE_NAMESPACE, "field");
    if !optional_sequence_is_valid(field, "pdfaType", xml, field_item_is_valid) {
        failed.insert(16);
    }
    for field in fields(value_type, PDFA_TYPE_NAMESPACE, "field") {
        if field.array_kind().is_some() {
            for item in field.array_items() {
                inspect_schema_field(item, xml, known_types, failed);
            }
        }
    }
}

fn inspect_schema_field(
    field: Node<'_, '_>,
    xml: &str,
    known_types: &BTreeSet<String>,
    failed: &mut BTreeSet<u8>,
) {
    if has_undefined_fields(
        field,
        PDFA_FIELD_NAMESPACE,
        &["name", "valueType", "description"],
    ) {
        failed.insert(1);
    }
    if !required_simple_field_has_prefix(field, PDFA_FIELD_NAMESPACE, "name", "pdfaField", xml) {
        failed.insert(17);
    }
    if !required_simple_field_has_prefix(field, PDFA_FIELD_NAMESPACE, "valueType", "pdfaField", xml)
        || !field_value(field, PDFA_FIELD_NAMESPACE, "valueType")
            .is_some_and(|value| known_types.contains(value.trim()))
    {
        failed.insert(18);
    }
    if !required_simple_field_has_prefix(
        field,
        PDFA_FIELD_NAMESPACE,
        "description",
        "pdfaField",
        xml,
    ) {
        failed.insert(19);
    }
}

fn child_properties<'a>(node: Node<'a, 'a>) -> Vec<XmpProperty<'a>> {
    node.attributes()
        .filter(|attribute| !matches!(attribute.namespace(), Some(RDF_NAMESPACE | XML_NAMESPACE)))
        .map(XmpProperty::Attribute)
        .chain(
            node.children()
                .filter(|child| child.is_element())
                .map(XmpProperty::Element),
        )
        .collect()
}

fn fields<'a>(node: Node<'a, 'a>, namespace: &str, name: &str) -> Vec<XmpProperty<'a>> {
    child_properties(node)
        .into_iter()
        .filter(|property| property.namespace() == Some(namespace) && property.name() == name)
        .collect()
}

fn first_field<'a>(node: Node<'a, 'a>, namespace: &str, name: &str) -> Option<XmpProperty<'a>> {
    fields(node, namespace, name).into_iter().next()
}

fn field_value<'a>(node: Node<'a, 'a>, namespace: &str, name: &str) -> Option<&'a str> {
    first_field(node, namespace, name).and_then(XmpProperty::value)
}

fn has_undefined_fields(node: Node<'_, '_>, namespace: &str, valid_names: &[&str]) -> bool {
    child_properties(node).into_iter().any(|property| {
        property.namespace() != Some(namespace) || !valid_names.contains(&property.name())
    })
}

fn required_simple_field_has_prefix(
    node: Node<'_, '_>,
    namespace: &str,
    name: &str,
    prefix: &str,
    xml: &str,
) -> bool {
    first_field(node, namespace, name)
        .is_some_and(|property| property.is_simple() && property.prefix(xml) == Some(prefix))
}

fn optional_sequence_is_valid(
    property: Option<XmpProperty<'_>>,
    prefix: &str,
    xml: &str,
    item_is_valid: impl Fn(Node<'_, '_>) -> bool,
) -> bool {
    let Some(property) = property else {
        return true;
    };
    sequence_items_are_valid(property, item_is_valid)
        && property.prefix(xml).is_none_or(|actual| actual == prefix)
}

fn sequence_items_are_valid(
    property: XmpProperty<'_>,
    item_is_valid: impl Fn(Node<'_, '_>) -> bool,
) -> bool {
    property.array_kind() == Some(ArrayKind::Seq)
        && property.array_items().into_iter().all(item_is_valid)
}

fn property_item_is_valid(property: Node<'_, '_>) -> bool {
    !has_undefined_fields(
        property,
        PDFA_PROPERTY_NAMESPACE,
        &["name", "valueType", "category", "description"],
    ) && ["name", "valueType", "category", "description"]
        .into_iter()
        .all(|name| {
            first_field(property, PDFA_PROPERTY_NAMESPACE, name).is_some_and(XmpProperty::is_simple)
        })
}

fn value_type_item_is_valid(value_type: Node<'_, '_>) -> bool {
    !has_undefined_fields(
        value_type,
        PDFA_TYPE_NAMESPACE,
        &["type", "namespaceURI", "prefix", "description", "field"],
    ) && ["type", "namespaceURI", "prefix", "description"]
        .into_iter()
        .all(|name| {
            first_field(value_type, PDFA_TYPE_NAMESPACE, name).is_some_and(XmpProperty::is_simple)
        })
}

fn field_item_is_valid(field: Node<'_, '_>) -> bool {
    !has_undefined_fields(
        field,
        PDFA_FIELD_NAMESPACE,
        &["name", "valueType", "description"],
    ) && ["name", "valueType", "description"]
        .into_iter()
        .all(|name| {
            first_field(field, PDFA_FIELD_NAMESPACE, name).is_some_and(XmpProperty::is_simple)
        })
}

fn known_types(definition: Node<'_, '_>) -> BTreeSet<String> {
    let mut known = [
        "AgentName",
        "Boolean",
        "Choice",
        "Date",
        "Integer",
        "Lang Alt",
        "Locale",
        "MIMEType",
        "ProperName",
        "Real",
        "RenditionClass",
        "ResourceEvent",
        "ResourceRef",
        "Text",
        "URI",
        "URL",
        "Version",
        "XPath",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    for value_type in fields(definition, PDFA_SCHEMA_NAMESPACE, "valueType") {
        for item in value_type.array_items() {
            if let Some(value) = field_value(item, PDFA_TYPE_NAMESPACE, "type") {
                known.insert(value.trim().to_owned());
            }
        }
    }
    known
}

fn xmp_packet_header<'a>(document: &'a Document<'a>) -> Option<&'a str> {
    let mut packet_header = None;
    for node in document.descendants() {
        if node.is_element()
            && node.tag_name().namespace() == Some(RDF_NAMESPACE)
            && node.tag_name().name() == "RDF"
        {
            break;
        }
        if let Some(instruction) = node.pi()
            && instruction.target == "xpacket"
        {
            packet_header = instruction.value;
        }
    }
    packet_header
}

fn has_quoted_assignment(header: &str, name: &[u8]) -> bool {
    let bytes = header.as_bytes();
    bytes
        .windows(name.len())
        .enumerate()
        .any(|(index, candidate)| {
            if candidate != name {
                return false;
            }
            let mut remainder = &bytes[index + name.len()..];
            remainder = trim_ascii_regex_whitespace(remainder);
            let Some(after_equals) = remainder.strip_prefix(b"=") else {
                return false;
            };
            remainder = trim_ascii_regex_whitespace(after_equals);
            let Some((&quote, value)) = remainder.split_first() else {
                return false;
            };
            matches!(quote, b'\'' | b'"') && value.contains(&quote)
        })
}

fn trim_ascii_regex_whitespace(mut bytes: &[u8]) -> &[u8] {
    while bytes
        .first()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r'))
    {
        bytes = &bytes[1..];
    }
    bytes
}

fn contains_namespace_property(document: &Document<'_>, namespace: &str) -> bool {
    document.descendants().any(|node| {
        node.is_element()
            && (node.tag_name().namespace() == Some(namespace)
                || node
                    .attributes()
                    .any(|attribute| attribute.namespace() == Some(namespace)))
    })
}

fn property_nodes<'a>(
    document: &'a Document<'a>,
    namespace: &str,
    local_name: &str,
) -> Vec<Node<'a, 'a>> {
    document
        .descendants()
        .filter(|node| {
            node.is_element()
                && node.tag_name().namespace() == Some(namespace)
                && node.tag_name().name() == local_name
        })
        .collect()
}

fn property_values(document: &Document<'_>, namespace: &str, local_name: &str) -> Vec<String> {
    let mut values = Vec::new();
    for node in document.descendants().filter(|node| node.is_element()) {
        if node.tag_name().namespace() == Some(namespace)
            && node.tag_name().name() == local_name
            && let Some(value) = direct_text(node)
        {
            values.push(value);
        }
        for attribute in node.attributes() {
            if attribute.namespace() == Some(namespace) && attribute.name() == local_name {
                values.push(attribute.value().trim().to_owned());
            }
        }
    }
    values
}

fn alt_values(
    document: &Document<'_>,
    namespace: &str,
    local_name: &str,
    language: &str,
) -> Vec<String> {
    property_nodes(document, namespace, local_name)
        .into_iter()
        .flat_map(|property| container_values(property, "Alt", Some(language)))
        .collect()
}

fn container_values(
    property: Node<'_, '_>,
    container_name: &str,
    language: Option<&str>,
) -> Vec<String> {
    property
        .children()
        .filter(|node| {
            node.is_element()
                && node.tag_name().namespace() == Some(RDF_NAMESPACE)
                && node.tag_name().name() == container_name
        })
        .flat_map(|container| {
            container.children().filter_map(|item| {
                if !item.is_element()
                    || item.tag_name().namespace() != Some(RDF_NAMESPACE)
                    || item.tag_name().name() != "li"
                    || language.is_some_and(|language| {
                        item.attribute((XML_NAMESPACE, "lang")) != Some(language)
                    })
                {
                    return None;
                }
                direct_text(item)
            })
        })
        .collect()
}

fn direct_text(node: Node<'_, '_>) -> Option<String> {
    let value = node
        .children()
        .filter(|child| child.is_text())
        .filter_map(|child| child.text())
        .collect::<String>();
    (!value.is_empty()).then(|| value.trim().to_owned())
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

pub(crate) fn dates_equivalent(pdf: &str, xmp: &str) -> bool {
    let Some(pdf) = ParsedDate::from_pdf(pdf) else {
        return false;
    };
    let Some(xmp) = ParsedDate::from_xmp(xmp) else {
        return false;
    };
    pdf.equivalent(xmp)
}

#[derive(Clone, Copy, Debug)]
struct ParsedDate {
    year: i32,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    offset_minutes: Option<i16>,
}

impl ParsedDate {
    fn from_pdf(value: &str) -> Option<Self> {
        let value = value.strip_prefix("D:")?;
        if value.len() < 4 {
            return None;
        }
        let year = digits(value, 0, 4)? as i32;
        let month = optional_digits(value, 4, 2, 1)?;
        let day = optional_digits(value, 6, 2, 1)?;
        let hour = optional_digits(value, 8, 2, 0)?;
        let minute = optional_digits(value, 10, 2, 0)?;
        let second = optional_digits(value, 12, 2, 0)?;
        let rest = value.get(value.len().min(14)..)?;
        let offset_minutes = parse_pdf_offset(rest)?;
        Self::checked(year, month, day, hour, minute, second, offset_minutes)
    }

    fn from_xmp(value: &str) -> Option<Self> {
        if value.len() < 19
            || value.as_bytes().get(4) != Some(&b'-')
            || value.as_bytes().get(7) != Some(&b'-')
            || value.as_bytes().get(10) != Some(&b'T')
            || value.as_bytes().get(13) != Some(&b':')
            || value.as_bytes().get(16) != Some(&b':')
        {
            return None;
        }
        let year = digits(value, 0, 4)? as i32;
        let month = digits(value, 5, 2)? as u8;
        let day = digits(value, 8, 2)? as u8;
        let hour = digits(value, 11, 2)? as u8;
        let minute = digits(value, 14, 2)? as u8;
        let second = digits(value, 17, 2)? as u8;
        let zone = &value[19..];
        let offset_minutes = match zone {
            "" => None,
            "Z" => Some(0),
            _ if zone.len() == 6
                && matches!(zone.as_bytes()[0], b'+' | b'-')
                && zone.as_bytes()[3] == b':' =>
            {
                let sign = if zone.as_bytes()[0] == b'-' { -1 } else { 1 };
                let hours = digits(zone, 1, 2)? as i16;
                let minutes = digits(zone, 4, 2)? as i16;
                (hours <= 23 && minutes <= 59).then_some(sign * (hours * 60 + minutes))
            }
            _ => None?,
        };
        Self::checked(year, month, day, hour, minute, second, offset_minutes)
    }

    fn checked(
        year: i32,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
        offset_minutes: Option<i16>,
    ) -> Option<Self> {
        if !(1..=12).contains(&month)
            || day == 0
            || day > days_in_month(year, month)
            || hour > 23
            || minute > 59
            || second > 59
        {
            return None;
        }
        Some(Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            offset_minutes,
        })
    }

    fn equivalent(self, other: Self) -> bool {
        match (self.offset_minutes, other.offset_minutes) {
            (Some(left), Some(right)) => {
                self.local_seconds() - i64::from(left) * 60
                    == other.local_seconds() - i64::from(right) * 60
            }
            (None, None) => {
                self.year == other.year
                    && self.month == other.month
                    && self.day == other.day
                    && self.hour == other.hour
                    && self.minute == other.minute
                    && self.second == other.second
            }
            // A missing zone is interpreted using host defaults by some XMP
            // stacks. Host-dependent validation is unsuitable here, so the
            // local subset refuses to guess.
            _ => false,
        }
    }

    fn local_seconds(self) -> i64 {
        days_from_civil(self.year, self.month, self.day) * 86_400
            + i64::from(self.hour) * 3_600
            + i64::from(self.minute) * 60
            + i64::from(self.second)
    }
}

fn digits(value: &str, start: usize, length: usize) -> Option<u32> {
    value.get(start..start + length)?.parse().ok()
}

fn optional_digits(value: &str, start: usize, length: usize, default: u8) -> Option<u8> {
    if value.len() <= start {
        Some(default)
    } else {
        digits(value, start, length).map(|value| value as u8)
    }
}

fn parse_pdf_offset(value: &str) -> Option<Option<i16>> {
    if value.is_empty() {
        return Some(Some(0));
    }
    if value == "Z" {
        return Some(Some(0));
    }
    let sign = match value.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let normalized = value[1..].replace('\'', "");
    if normalized.len() != 4 {
        return None;
    }
    let hours = digits(&normalized, 0, 2)? as i16;
    let minutes = digits(&normalized, 2, 2)? as i16;
    (hours <= 23 && minutes <= 59).then_some(Some(sign * (hours * 60 + minutes)))
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 31,
    }
}

// Howard Hinnant's proleptic-Gregorian civil-date conversion, translated to
// Rust. Only equality matters, so the arbitrary epoch constant is omitted.
fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let adjusted_year = year - i32::from(month <= 2);
    let era = if adjusted_year >= 0 {
        adjusted_year
    } else {
        adjusted_year - 399
    } / 400;
    let year_of_era = adjusted_year - era * 400;
    let shifted_month = i32::from(month) + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + i32::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    i64::from(era * 146_097 + day_of_era)
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPLETE_XMP: &[u8] = br#"<rdf:RDF
      xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
      xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/"
      xmlns:dc="http://purl.org/dc/elements/1.1/"
      xmlns:pdf="http://ns.adobe.com/pdf/1.3/"
      xmlns:xmp="http://ns.adobe.com/xap/1.0/">
      <rdf:Description pdfaid:part="1" pdfaid:conformance="B"
        pdf:Keywords="rust,pdf" pdf:Producer="producer"
        xmp:CreatorTool="tool" xmp:CreateDate="2026-07-27T12:30:45+02:00"
        xmp:ModifyDate="2026-07-27T12:30:45+02:00">
        <dc:title><rdf:Alt><rdf:li xml:lang="fr">Titre</rdf:li>
          <rdf:li xml:lang="x-default">Title</rdf:li></rdf:Alt></dc:title>
        <dc:creator><rdf:Seq><rdf:li>Author</rdf:li></rdf:Seq></dc:creator>
        <dc:description><rdf:Alt><rdf:li xml:lang="x-default">Subject</rdf:li>
          </rdf:Alt></dc:description>
      </rdf:Description>
    </rdf:RDF>"#;

    #[test]
    fn parses_typed_standard_properties() {
        let parsed = parse_xmp(COMPLETE_XMP).expect("valid XMP");
        assert_eq!(parsed.pdfa_parts, ["1"]);
        assert_eq!(parsed.pdfa_conformances, ["B"]);
        assert_eq!(parsed.title_x_default, ["Title"]);
        assert_eq!(parsed.creators, ["Author"]);
        assert_eq!(parsed.creator_container_count, 1);
        assert_eq!(parsed.description_x_default, ["Subject"]);
        assert_eq!(parsed.keywords, ["rust,pdf"]);
        assert_eq!(parsed.creator_tools, ["tool"]);
        assert_eq!(parsed.producers, ["producer"]);
    }

    #[test]
    fn retains_duplicate_identification_values() {
        let xmp = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/">
          <rdf:Description pdfaid:part="1" pdfaid:conformance="B"/>
          <rdf:Description pdfaid:part="2" pdfaid:conformance="A"/>
        </rdf:RDF>"#;
        let parsed = parse_xmp(xmp).expect("valid XMP");
        assert_eq!(parsed.pdfa_parts, ["1", "2"]);
        assert_eq!(parsed.pdfa_conformances, ["B", "A"]);
    }

    #[test]
    fn detects_any_pdfa_identification_namespace_property() {
        let xmp = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/">
          <rdf:Description pdfaid:amd="1:2026"/>
        </rdf:RDF>"#;
        let parsed = parse_xmp(xmp).expect("valid XMP");
        assert!(parsed.pdfa_identification_present);
        assert!(parsed.pdfa_parts.is_empty());
        assert!(parsed.pdfa_conformances.is_empty());
    }

    #[test]
    fn rejects_malformed_xmp_and_dtd() {
        assert!(parse_xmp(b"<rdf:RDF>").is_err());
        assert!(parse_xmp(b"<!DOCTYPE x><x/>").is_err());
    }

    #[test]
    fn packet_header_attributes_follow_the_pinned_matching_model() {
        let parse = |header: &str| {
            parse_xmp(
                format!(
                    "<?xpacket {header}?><rdf:RDF xmlns:rdf=\"{RDF_NAMESPACE}\"/>\
                     <?xpacket end=\"w\"?>"
                )
                .as_bytes(),
            )
            .expect("valid packet")
        };
        let bytes = parse("begin=\"\" bytes = '123'");
        assert!(bytes.packet_header_has_bytes);
        assert!(!bytes.packet_header_has_encoding);

        let both = parse("begin=\"\" mybytes=\"123\" encoding=\"UTF-8\"");
        assert!(both.packet_header_has_bytes);
        assert!(both.packet_header_has_encoding);

        let ignored = parse("begin=\"\" Bytes=\"123\" bytes=123");
        assert!(!ignored.packet_header_has_bytes);
        assert!(!ignored.packet_header_has_encoding);
    }

    #[test]
    fn compares_supported_pdf_and_xmp_dates() {
        assert!(dates_equivalent(
            "D:20260727123045+02'00'",
            "2026-07-27T12:30:45+02:00"
        ));
        assert!(dates_equivalent(
            "D:20260727123045+02'00'",
            "2026-07-27T10:30:45Z"
        ));
        assert!(dates_equivalent("D:20260727123045", "2026-07-27T12:30:45Z"));
        assert!(!dates_equivalent("20260727123045Z", "2026-07-27T12:30:45Z"));
        assert!(!dates_equivalent(
            "D:20261327123045+02'00'",
            "2026-07-27T12:30:45+02:00"
        ));
    }
}
