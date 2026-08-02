use std::collections::{BTreeMap, BTreeSet};

use chrono::{Duration, Local, LocalResult, NaiveDate, TimeZone};
use roxmltree::{Attribute, Document, Node, ParsingOptions};
use serde::Serialize;
use sxd_xpath::Factory as XPathFactory;

const RDF_NAMESPACE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
const PDFA_ID_NAMESPACE: &str = "http://www.aiim.org/pdfa/ns/id/";
const DC_NAMESPACE: &str = "http://purl.org/dc/elements/1.1/";
const DC_DEPRECATED_NAMESPACE: &str = "http://purl.org/dc/1.1/";
const PDF_NAMESPACE: &str = "http://ns.adobe.com/pdf/1.3/";
const XMP_NAMESPACE: &str = "http://ns.adobe.com/xap/1.0/";
const PDFA_EXTENSION_NAMESPACE: &str = "http://www.aiim.org/pdfa/ns/extension/";
const PDFA_SCHEMA_NAMESPACE: &str = "http://www.aiim.org/pdfa/ns/schema#";
const PDFA_PROPERTY_NAMESPACE: &str = "http://www.aiim.org/pdfa/ns/property#";
const PDFA_TYPE_NAMESPACE: &str = "http://www.aiim.org/pdfa/ns/type#";
const PDFA_FIELD_NAMESPACE: &str = "http://www.aiim.org/pdfa/ns/field#";
const EXIF_NAMESPACE: &str = "http://ns.adobe.com/exif/1.0/";
const XMP_DIMENSIONS_NAMESPACE: &str = "http://ns.adobe.com/xap/1.0/sType/Dimensions#";
const XMP_JOB_NAMESPACE: &str = "http://ns.adobe.com/xap/1.0/sType/Job#";
const XMP_RESOURCE_EVENT_NAMESPACE: &str = "http://ns.adobe.com/xap/1.0/sType/ResourceEvent#";
const XMP_RESOURCE_REF_NAMESPACE: &str = "http://ns.adobe.com/xap/1.0/sType/ResourceRef#";
const XMP_THUMBNAIL_NAMESPACE: &str = "http://ns.adobe.com/xap/1.0/g/img/";
const XMP_VERSION_NAMESPACE: &str = "http://ns.adobe.com/xap/1.0/sType/Version#";
const MAX_XMP_XML_NODES: u32 = 100_000;
const MAX_XMP_XML_DEPTH: usize = 128;

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
    #[serde(skip)]
    pub packet_header_has_bytes: bool,
    #[serde(skip)]
    pub packet_header_has_encoding: bool,
    #[serde(skip)]
    pub extension_schema_failed_tests: BTreeSet<u8>,
    #[serde(skip)]
    pub invalid_predefined_xmp_properties: BTreeSet<String>,
    #[serde(skip)]
    pub invalid_predefined_xmp_value_types: BTreeSet<String>,
    #[serde(skip)]
    pub undefined_extension_xmp_properties: BTreeSet<String>,
    #[serde(skip)]
    pub invalid_extension_xmp_value_types: BTreeSet<String>,
    #[serde(skip)]
    pub identification_prefix_failed_tests: BTreeSet<u8>,
}

pub(crate) fn parse_xmp(bytes: &[u8]) -> Result<XmpMetadata, String> {
    let mut xml = decode_xml(bytes)?;
    preflight_xml_depth(&xml)?;
    if !xmp_xml_parses(&xml) {
        repair_xml_controls(&mut xml);
        preflight_xml_depth(&xml)?;
    }
    let document = Document::parse_with_options(
        &xml,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_XMP_XML_NODES,
            ..ParsingOptions::default()
        },
    )
    .map_err(|error| error.to_string())?;
    validate_xml_depth(&document)?;
    let Some((rdf, packet_header)) = select_xmp_root(&document) else {
        return Ok(XmpMetadata::default());
    };
    validate_rdf_package(rdf, &xml)?;
    let extension_schema_failed_tests = inspect_extension_schemas(rdf, &xml);
    let invalid_predefined_xmp_properties = inspect_predefined_xmp_properties(rdf, &xml);
    let invalid_predefined_xmp_value_types = inspect_predefined_xmp_value_types(rdf, &xml);
    let extension_schema_definitions = extension_schema_property_definitions(rdf);
    let undefined_extension_xmp_properties =
        inspect_undefined_extension_xmp_properties(rdf, &xml, &extension_schema_definitions);
    let invalid_extension_xmp_value_types =
        inspect_extension_xmp_value_types(rdf, &xml, &extension_schema_definitions);
    let identification_prefix_failed_tests = inspect_identification_prefixes(rdf, &xml);

    let pdfa_identification_present = contains_namespace_property(rdf, &xml, PDFA_ID_NAMESPACE);
    let pdfa_parts = property_values(rdf, &xml, PDFA_ID_NAMESPACE, "part");
    let pdfa_conformances = property_values(rdf, &xml, PDFA_ID_NAMESPACE, "conformance");
    let title_x_default = localized_text_values(rdf, DC_NAMESPACE, "title");
    let creator_nodes = property_nodes(rdf, DC_NAMESPACE, "creator");
    let creators = creator_nodes
        .iter()
        .flat_map(|node| ordered_array_values(*node).unwrap_or_default())
        .collect();
    let description_x_default = localized_text_values(rdf, DC_NAMESPACE, "description");

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
        keywords: property_values(rdf, &xml, PDF_NAMESPACE, "Keywords"),
        creator_tools: property_values(rdf, &xml, XMP_NAMESPACE, "CreatorTool"),
        producers: property_values(rdf, &xml, PDF_NAMESPACE, "Producer"),
        create_dates: property_values(rdf, &xml, XMP_NAMESPACE, "CreateDate"),
        modify_dates: property_values(rdf, &xml, XMP_NAMESPACE, "ModifyDate"),
        packet_header_has_bytes: packet_header
            .is_some_and(|header| has_quoted_assignment(header, b"bytes")),
        packet_header_has_encoding: packet_header
            .is_some_and(|header| has_quoted_assignment(header, b"encoding")),
        extension_schema_failed_tests,
        invalid_predefined_xmp_properties,
        invalid_predefined_xmp_value_types,
        undefined_extension_xmp_properties,
        invalid_extension_xmp_value_types,
        identification_prefix_failed_tests,
    })
}

fn xmp_xml_parses(xml: &str) -> bool {
    Document::parse_with_options(
        xml,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_XMP_XML_NODES,
            ..ParsingOptions::default()
        },
    )
    .is_ok()
}

fn preflight_xml_depth(xml: &str) -> Result<(), String> {
    let bytes = xml.as_bytes();
    let mut position = 0_usize;
    let mut depth = 0_usize;
    while let Some(relative) = bytes[position..].iter().position(|byte| *byte == b'<') {
        position += relative;
        let tail = &bytes[position..];
        if tail.starts_with(b"<!--") {
            position = find_xml_delimiter(bytes, position + 4, b"-->")?;
            continue;
        }
        if tail.starts_with(b"<![CDATA[") {
            position = find_xml_delimiter(bytes, position + 9, b"]]>")?;
            continue;
        }
        if tail.starts_with(b"<?") {
            position = find_xml_delimiter(bytes, position + 2, b"?>")?;
            continue;
        }
        if tail.starts_with(b"<!") {
            return Err(
                "DTD and XML declarations other than comments or CDATA are disabled".to_owned(),
            );
        }
        let (end, self_closing) = find_xml_tag_end(bytes, position + 1)?;
        if tail.starts_with(b"</") {
            depth = depth.saturating_sub(1);
        } else if !self_closing {
            depth += 1;
            if depth > MAX_XMP_XML_DEPTH {
                return Err(format!("XMP XML nesting depth exceeds {MAX_XMP_XML_DEPTH}"));
            }
        }
        position = end;
    }
    Ok(())
}

fn find_xml_delimiter(bytes: &[u8], start: usize, delimiter: &[u8]) -> Result<usize, String> {
    bytes[start..]
        .windows(delimiter.len())
        .position(|window| window == delimiter)
        .map(|relative| start + relative + delimiter.len())
        .ok_or_else(|| "unterminated XML construct".to_owned())
}

fn find_xml_tag_end(bytes: &[u8], start: usize) -> Result<(usize, bool), String> {
    let mut quote = None;
    let mut position = start;
    while let Some(byte) = bytes.get(position).copied() {
        match (quote, byte) {
            (Some(expected), actual) if expected == actual => quote = None,
            (None, b'\'' | b'"') => quote = Some(byte),
            (None, b'>') => {
                let self_closing = bytes[start..position]
                    .iter()
                    .rev()
                    .find(|byte| !byte.is_ascii_whitespace())
                    == Some(&b'/');
                return Ok((position + 1, self_closing));
            }
            _ => {}
        }
        position += 1;
    }
    Err("unterminated XML tag".to_owned())
}

fn validate_xml_depth(document: &Document<'_>) -> Result<(), String> {
    let mut stack = vec![(document.root(), 0_usize)];
    while let Some((node, depth)) = stack.pop() {
        if depth > MAX_XMP_XML_DEPTH {
            return Err(format!("XMP XML nesting depth exceeds {MAX_XMP_XML_DEPTH}"));
        }
        stack.extend(
            node.children()
                .map(|child| (child, depth + usize::from(child.is_element()))),
        );
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RdfCompoundKind {
    Schema,
    Struct,
    Array,
}

#[derive(Default)]
struct RdfNodeState {
    kind: Option<RdfCompoundKind>,
    children: BTreeSet<(String, String)>,
    qualifiers: BTreeSet<(String, String)>,
    value_child: Option<Box<RdfNodeState>>,
}

impl RdfNodeState {
    fn new(kind: RdfCompoundKind) -> Self {
        Self {
            kind: Some(kind),
            ..Self::default()
        }
    }

    fn add_child(&mut self, namespace: Option<&str>, name: &str) -> Result<bool, String> {
        let Some(namespace) = namespace else {
            return Err(format!("property {name:?} has no XML namespace"));
        };
        let namespace = normalized_xmp_namespace(namespace);
        let is_array_item = namespace == RDF_NAMESPACE && name == "li";
        let is_value = namespace == RDF_NAMESPACE && name == "value";
        if is_array_item && self.kind != Some(RdfCompoundKind::Array) {
            return Err("misplaced rdf:li element".to_owned());
        }
        if is_value && self.kind != Some(RdfCompoundKind::Struct) {
            return Err("misplaced rdf:value element".to_owned());
        }
        if !is_array_item
            && !self
                .children
                .insert((namespace.to_owned(), name.to_owned()))
        {
            return Err(format!(
                "XMP property {{{namespace}}}{name} is declared more than once"
            ));
        }
        Ok(is_value)
    }

    fn add_qualifier(&mut self, namespace: &str, name: &str) -> Result<(), String> {
        if !self
            .qualifiers
            .insert((namespace.to_owned(), name.to_owned()))
        {
            return Err(format!("duplicate qualifier {{{namespace}}}{name}"));
        }
        Ok(())
    }

    fn fixup_qualified_value(&mut self) -> Result<(), String> {
        let Some(value) = self.value_child.as_ref() else {
            return Ok(());
        };
        for qualifier in &value.qualifiers {
            if !self.qualifiers.insert(qualifier.clone()) {
                return Err("duplicate qualifier on rdf:value property".to_owned());
            }
        }
        for child in self
            .children
            .iter()
            .filter(|(namespace, name)| namespace != RDF_NAMESPACE || name != "value")
        {
            if !self.qualifiers.insert(child.clone()) {
                return Err("duplicate field and qualifier on rdf:value property".to_owned());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RdfTerm {
    Other,
    Rdf,
    Id,
    About,
    ParseType,
    Resource,
    NodeId,
    Datatype,
    Description,
    Li,
    Old,
}

/// Validate the complete RDF/XML serialization subset accepted by the Adobe
/// XMP 2004 parser embedded in veraPDF 1.28.2.
fn validate_rdf_package(rdf: Node<'_, '_>, xml: &str) -> Result<(), String> {
    validate_cdata_usage(rdf, xml)?;
    let mut package = RdfNodeState::new(RdfCompoundKind::Schema);
    let mut about = None;
    validate_property_element_list(rdf, true, false, &mut package, &mut about, xml)
}

fn validate_property_element_list(
    parent: Node<'_, '_>,
    node_element_list: bool,
    properties_are_top_level: bool,
    state: &mut RdfNodeState,
    package_about: &mut Option<String>,
    xml: &str,
) -> Result<(), String> {
    for child in parent.children() {
        if child.is_comment()
            || child.is_text() && child.text().is_none_or(java_text_is_whitespace)
        {
            continue;
        }
        if !child.is_element() {
            return Err("expected RDF element".to_owned());
        }
        if node_element_list {
            validate_node_element(child, true, state, package_about, xml)?;
        } else {
            validate_property_element(child, properties_are_top_level, state, package_about, xml)?;
        }
    }
    Ok(())
}

fn validate_node_element(
    node: Node<'_, '_>,
    top_level: bool,
    state: &mut RdfNodeState,
    package_about: &mut Option<String>,
    xml: &str,
) -> Result<(), String> {
    let term = rdf_term(node.tag_name().namespace(), node.tag_name().name(), false);
    if !matches!(term, RdfTerm::Description | RdfTerm::Other) {
        return Err("node element must be rdf:Description or a typed node".to_owned());
    }
    if top_level && term == RdfTerm::Other {
        return Err("top-level typed RDF node is not allowed".to_owned());
    }
    let mut exclusive = false;
    for attribute in node.attributes() {
        match rdf_attribute_term(node, attribute) {
            RdfTerm::Id | RdfTerm::NodeId | RdfTerm::About => {
                if exclusive {
                    return Err(
                        "mutually exclusive rdf:about, rdf:ID, or rdf:nodeID attributes".to_owned(),
                    );
                }
                exclusive = true;
                if top_level
                    && matches!(rdf_attribute_term(node, attribute), RdfTerm::About)
                    && !attribute.value().is_empty()
                {
                    if package_about
                        .as_deref()
                        .is_some_and(|about| about != attribute.value())
                    {
                        return Err("mismatched top-level rdf:about values".to_owned());
                    }
                    *package_about = Some(attribute.value().to_owned());
                }
            }
            RdfTerm::Other => {
                state.add_child(attribute.namespace(), attribute.name())?;
            }
            _ => return Err("invalid RDF node-element attribute".to_owned()),
        }
    }
    validate_property_element_list(node, false, top_level, state, package_about, xml)
}

fn validate_property_element(
    property: Node<'_, '_>,
    is_top_level: bool,
    parent: &mut RdfNodeState,
    package_about: &mut Option<String>,
    xml: &str,
) -> Result<(), String> {
    if is_top_level && is_ignored_ix_changes_property(property, xml) {
        return Ok(());
    }
    let term = rdf_term(
        property.tag_name().namespace(),
        property.tag_name().name(),
        false,
    );
    if matches!(term, RdfTerm::Description | RdfTerm::Old)
        || matches!(
            term,
            RdfTerm::Rdf
                | RdfTerm::Id
                | RdfTerm::About
                | RdfTerm::ParseType
                | RdfTerm::Resource
                | RdfTerm::NodeId
                | RdfTerm::Datatype
        )
    {
        return Err("invalid RDF property-element name".to_owned());
    }

    let attributes = property.attributes().collect::<Vec<_>>();
    if attributes.len() > 3 {
        return validate_empty_property(property, parent);
    }
    for attribute in &attributes {
        if attribute.namespace() == Some(XML_NAMESPACE) && attribute.name() == "lang" {
            continue;
        }
        match rdf_attribute_term(property, *attribute) {
            RdfTerm::Datatype => return validate_literal_property(property, parent),
            RdfTerm::ParseType => {
                return match attribute.value() {
                    "Resource" => {
                        validate_parse_type_resource(property, parent, package_about, xml)
                    }
                    "Literal" => Err("rdf:parseType Literal is not allowed in XMP".to_owned()),
                    "Collection" => {
                        Err("rdf:parseType Collection is not allowed in XMP".to_owned())
                    }
                    _ => Err("unsupported rdf:parseType value".to_owned()),
                };
            }
            _ => return validate_empty_property(property, parent),
        }
    }
    if property
        .children()
        .any(|child| !child.is_text() && !child.is_comment())
    {
        validate_resource_property(property, parent, package_about, xml)
    } else if property.children().next().is_some() {
        validate_literal_property(property, parent)
    } else {
        validate_empty_property(property, parent)
    }
}

fn validate_literal_property(
    property: Node<'_, '_>,
    parent: &mut RdfNodeState,
) -> Result<(), String> {
    let is_value = parent.add_child(property.tag_name().namespace(), property.tag_name().name())?;
    let mut state = RdfNodeState::default();
    for attribute in property.attributes() {
        if attribute.namespace() == Some(XML_NAMESPACE) && attribute.name() == "lang" {
            state.add_qualifier(XML_NAMESPACE, "lang")?;
        } else if attribute.namespace() == Some(RDF_NAMESPACE)
            && matches!(attribute.name(), "ID" | "datatype")
        {
            continue;
        } else {
            return Err("invalid literal-property attribute".to_owned());
        }
    }
    if property
        .children()
        .any(|child| !child.is_text() && !child.is_comment())
    {
        return Err("invalid child of literal RDF property".to_owned());
    }
    if is_value {
        parent.value_child = Some(Box::new(state));
    }
    Ok(())
}

fn validate_resource_property(
    property: Node<'_, '_>,
    parent: &mut RdfNodeState,
    package_about: &mut Option<String>,
    xml: &str,
) -> Result<(), String> {
    let is_value = parent.add_child(property.tag_name().namespace(), property.tag_name().name())?;
    let mut language_qualifier = false;
    for attribute in property.attributes() {
        if attribute.namespace() == Some(XML_NAMESPACE) && attribute.name() == "lang" {
            language_qualifier = true;
            continue;
        }
        if attribute.namespace() == Some(RDF_NAMESPACE) && attribute.name() == "ID" {
            continue;
        }
        return Err("invalid resource-property attribute".to_owned());
    }
    let children = property
        .children()
        .filter(|child| {
            !child.is_comment()
                && !(child.is_text() && child.text().is_none_or(java_text_is_whitespace))
        })
        .collect::<Vec<_>>();
    if children.len() != 1 || !children[0].is_element() {
        return Err("resource property must contain exactly one RDF node element".to_owned());
    }
    let child = children[0];
    let kind = if child.tag_name().namespace() == Some(RDF_NAMESPACE)
        && matches!(child.tag_name().name(), "Bag" | "Seq" | "Alt")
    {
        RdfCompoundKind::Array
    } else {
        RdfCompoundKind::Struct
    };
    let mut state = RdfNodeState::new(kind);
    if language_qualifier {
        state.add_qualifier(XML_NAMESPACE, "lang")?;
    }
    if kind == RdfCompoundKind::Struct
        && child.tag_name().namespace() != Some(RDF_NAMESPACE)
        && child.tag_name().name() != "Description"
    {
        state.add_qualifier(RDF_NAMESPACE, "type")?;
    }
    validate_node_element(child, false, &mut state, package_about, xml)?;
    state.fixup_qualified_value()?;
    if is_value {
        parent.value_child = Some(Box::new(state));
    }
    Ok(())
}

fn validate_parse_type_resource(
    property: Node<'_, '_>,
    parent: &mut RdfNodeState,
    package_about: &mut Option<String>,
    xml: &str,
) -> Result<(), String> {
    let is_value = parent.add_child(property.tag_name().namespace(), property.tag_name().name())?;
    let mut state = RdfNodeState::new(RdfCompoundKind::Struct);
    for attribute in property.attributes() {
        if attribute.namespace() == Some(XML_NAMESPACE) && attribute.name() == "lang" {
            state.add_qualifier(XML_NAMESPACE, "lang")?;
        } else if attribute.namespace() == Some(RDF_NAMESPACE)
            && matches!(attribute.name(), "ID" | "parseType")
        {
            continue;
        } else {
            return Err("invalid rdf:parseType Resource attribute".to_owned());
        }
    }
    validate_property_element_list(property, false, false, &mut state, package_about, xml)?;
    state.fixup_qualified_value()?;
    if is_value {
        parent.value_child = Some(Box::new(state));
    }
    Ok(())
}

fn validate_empty_property(
    property: Node<'_, '_>,
    parent: &mut RdfNodeState,
) -> Result<(), String> {
    if property.children().any(|child| !child.is_comment()) {
        return Err("nested content is not allowed on an empty RDF property".to_owned());
    }
    let mut has_property_attributes = false;
    let mut has_resource = false;
    let mut has_node_id = false;
    let mut value_attribute = None;
    for attribute in property.attributes() {
        match rdf_attribute_term(property, attribute) {
            RdfTerm::Id => {}
            RdfTerm::Resource => {
                if has_node_id || value_attribute.is_some() {
                    return Err("conflicting rdf:resource, rdf:nodeID, or rdf:value".to_owned());
                }
                has_resource = true;
                value_attribute = Some(attribute);
            }
            RdfTerm::NodeId => {
                if has_resource {
                    return Err("conflicting rdf:resource and rdf:nodeID".to_owned());
                }
                has_node_id = true;
            }
            RdfTerm::Other => {
                if attribute.namespace() == Some(RDF_NAMESPACE) && attribute.name() == "value" {
                    if has_resource {
                        return Err("conflicting rdf:value and rdf:resource".to_owned());
                    }
                    value_attribute = Some(attribute);
                } else if !(attribute.namespace() == Some(XML_NAMESPACE)
                    && attribute.name() == "lang")
                {
                    has_property_attributes = true;
                }
            }
            _ => return Err("unrecognized empty-property attribute".to_owned()),
        }
    }
    let is_value = parent.add_child(property.tag_name().namespace(), property.tag_name().name())?;
    let mut state = if value_attribute.is_none() && has_property_attributes {
        RdfNodeState::new(RdfCompoundKind::Struct)
    } else {
        RdfNodeState::default()
    };
    for attribute in property.attributes() {
        if value_attribute.is_some_and(|value| value == attribute) {
            continue;
        }
        match rdf_attribute_term(property, attribute) {
            RdfTerm::Id | RdfTerm::NodeId => {}
            RdfTerm::Resource => state.add_qualifier(RDF_NAMESPACE, "resource")?,
            RdfTerm::Other => {
                if state.kind == Some(RdfCompoundKind::Struct)
                    && !(attribute.namespace() == Some(XML_NAMESPACE) && attribute.name() == "lang")
                {
                    state.add_child(attribute.namespace(), attribute.name())?;
                } else {
                    let Some(namespace) = attribute.namespace() else {
                        return Err("qualifier has no XML namespace".to_owned());
                    };
                    state.add_qualifier(namespace, attribute.name())?;
                }
            }
            _ => return Err("unrecognized empty-property attribute".to_owned()),
        }
    }
    if is_value {
        parent.value_child = Some(Box::new(state));
    }
    Ok(())
}

fn rdf_attribute_term(owner: Node<'_, '_>, attribute: Attribute<'_, '_>) -> RdfTerm {
    let namespace = if attribute.namespace().is_none()
        && matches!(attribute.name(), "about" | "ID")
        && owner.tag_name().namespace() == Some(RDF_NAMESPACE)
    {
        Some(RDF_NAMESPACE)
    } else {
        attribute.namespace()
    };
    rdf_term(namespace, attribute.name(), true)
}

fn rdf_term(namespace: Option<&str>, name: &str, _attribute: bool) -> RdfTerm {
    if namespace != Some(RDF_NAMESPACE) {
        return RdfTerm::Other;
    }
    match name {
        "RDF" => RdfTerm::Rdf,
        "ID" => RdfTerm::Id,
        "about" => RdfTerm::About,
        "parseType" => RdfTerm::ParseType,
        "resource" => RdfTerm::Resource,
        "nodeID" => RdfTerm::NodeId,
        "datatype" => RdfTerm::Datatype,
        "Description" => RdfTerm::Description,
        "li" => RdfTerm::Li,
        "aboutEach" | "aboutEachPrefix" | "bagID" => RdfTerm::Old,
        _ => RdfTerm::Other,
    }
}

fn inspect_predefined_xmp_properties(rdf: Node<'_, '_>, xml: &str) -> BTreeSet<String> {
    let mut invalid = BTreeSet::new();
    for property in xmp_properties(rdf, xml) {
        if let Some(namespace) = property.namespace() {
            insert_invalid_predefined_property(&mut invalid, namespace, property.name());
        }
    }
    invalid
}

fn insert_invalid_predefined_property(
    properties: &mut BTreeSet<String>,
    namespace: &str,
    name: &str,
) {
    if predefined_xmp2004_namespace(namespace) && predefined_xmp2004_type(namespace, name).is_none()
    {
        properties.insert(format!("{{{namespace}}}{name}"));
    }
}

fn predefined_xmp2004_namespace(namespace: &str) -> bool {
    include_str!("xmp2004_properties.txt")
        .lines()
        .any(|line| line.starts_with(&format!("{{{namespace}}}")))
}

fn predefined_xmp2004_type(namespace: &str, name: &str) -> Option<&'static str> {
    include_str!("xmp2004_properties.txt")
        .lines()
        .find_map(|line| {
            let (property, value_type) = line.split_once('=')?;
            (property == format!("{{{namespace}}}{name}")).then_some(value_type)
        })
}

fn inspect_predefined_xmp_value_types(rdf: Node<'_, '_>, xml: &str) -> BTreeSet<String> {
    let mut invalid = BTreeSet::new();
    for property in xmp_properties(rdf, xml) {
        inspect_predefined_xmp_value_type(&mut invalid, property);
    }
    invalid
}

fn inspect_predefined_xmp_value_type(properties: &mut BTreeSet<String>, property: XmpProperty<'_>) {
    let Some(namespace) = property.namespace() else {
        return;
    };
    let Some(value_type) = predefined_xmp2004_type(namespace, property.name()) else {
        if predefined_xmp2004_namespace(namespace) {
            properties.insert(format!("{{{namespace}}}{} (undefined)", property.name()));
        }
        return;
    };
    if !predefined_value_type_matches(property, value_type) {
        properties.insert(format!("{{{namespace}}}{} ({value_type})", property.name()));
    }
}

fn predefined_value_type_matches(property: XmpProperty<'_>, value_type: &str) -> bool {
    value_type_matches(property, value_type, None)
}

fn value_type_matches(
    property: XmpProperty<'_>,
    value_type: &str,
    extension_types: Option<&BTreeMap<String, ExtensionTypeDefinition>>,
) -> bool {
    let value_type = xmp_type_key(value_type);
    let value_type = value_type.as_str();
    let (expected_array, item_type) = if let Some(item_type) = value_type.strip_prefix("bag ") {
        (Some(ArrayKind::Bag), item_type)
    } else if let Some(item_type) = value_type.strip_prefix("seq ") {
        (Some(ArrayKind::Seq), item_type)
    } else if let Some(item_type) = value_type.strip_prefix("alt ") {
        (Some(ArrayKind::Alt), item_type)
    } else if value_type == "lang alt" {
        (Some(ArrayKind::Alt), "lang alt")
    } else {
        (None, value_type)
    };
    if let Some(expected_array) = expected_array {
        if property.array_kind() != Some(expected_array) {
            return false;
        }
        if item_type == "lang alt" {
            let items = property.array_items();
            return items.is_empty()
                || items
                    .iter()
                    .any(|item| item.attribute((XML_NAMESPACE, "lang")).is_some());
        }
        return property.array_items().into_iter().all(|item| {
            value_type_matches(XmpProperty::Element(item), item_type, extension_types)
        });
    }
    if item_type == "any" {
        return true;
    }
    if let Some(definition) = extension_types.and_then(|types| types.get(item_type)) {
        return extension_xmp_value_matches(property, definition, extension_types);
    }
    if let Some((namespace, fields)) = structured_xmp_type(item_type) {
        return structured_xmp_value_matches(property, namespace, fields, extension_types);
    }
    property.is_simple()
        && matches!(
            item_type,
            "agentname"
                | "boolean"
                | "date"
                | "gpscoordinate"
                | "integer"
                | "locale"
                | "mimetype"
                | "propername"
                | "rational"
                | "real"
                | "renditionclass"
                | "text"
                | "uri"
                | "url"
                | "xpath"
        )
        && scalar_xmp_value_matches(property.value().unwrap_or(""), item_type)
}

fn xmp_type_key(value_type: &str) -> String {
    let lower = value_type.to_lowercase();
    let mut value_type = String::with_capacity(lower.len());
    let mut copied = 0;
    for (choice, _) in lower.match_indices("choice ") {
        if choice < copied {
            continue;
        }
        let start = if lower[..choice].ends_with("open ") {
            choice - "open ".len()
        } else if lower[..choice].ends_with("closed ") {
            choice - "closed ".len()
        } else {
            choice
        };
        value_type.push_str(&lower[copied..start]);
        let after_choice = choice + "choice ".len();
        copied = after_choice
            + usize::from(lower[after_choice..].starts_with("of ")) * "of ".len();
    }
    value_type.push_str(&lower[copied..]);
    if value_type.ends_with("choice") {
        let choice = value_type.len() - "choice".len();
        let start = if value_type[..choice].ends_with("open ") {
            choice - "open ".len()
        } else if value_type[..choice].ends_with("closed ") {
            choice - "closed ".len()
        } else {
            choice
        };
        value_type.truncate(start);
    }
    let value_type = java_string_trim(&value_type).to_owned();
    if value_type.is_empty() {
        return "text".to_owned();
    }
    if value_type.ends_with("lang alt") {
        return value_type;
    }
    for array_kind in ["bag", "seq", "alt"] {
        if value_type.ends_with(array_kind) {
            return format!("{value_type} text");
        }
    }
    value_type
}

fn structured_xmp_type(
    value_type: &str,
) -> Option<(&'static str, &'static [(&'static str, &'static str)])> {
    Some(match value_type {
        "thumbnail" => (
            XMP_THUMBNAIL_NAMESPACE,
            &[
                ("width", "integer"),
                ("format", "text"),
                ("image", "text"),
                ("height", "integer"),
            ],
        ),
        "resourceevent" => (
            XMP_RESOURCE_EVENT_NAMESPACE,
            &[
                ("softwareAgent", "agentname"),
                ("action", "text"),
                ("instanceID", "uri"),
                ("parameters", "text"),
                ("when", "date"),
            ],
        ),
        "resourceref" => (
            XMP_RESOURCE_REF_NAMESPACE,
            &[
                ("managerVariant", "text"),
                ("manageUI", "uri"),
                ("versionID", "text"),
                ("instanceID", "uri"),
                ("manager", "agentname"),
                ("renditionParams", "text"),
                ("manageTo", "uri"),
                ("documentID", "uri"),
                ("renditionClass", "renditionclass"),
            ],
        ),
        "version" => (
            XMP_VERSION_NAMESPACE,
            &[
                ("comments", "text"),
                ("modifyDate", "date"),
                ("event", "resourceevent"),
                ("version", "text"),
                ("modifier", "propername"),
            ],
        ),
        "job" => (
            XMP_JOB_NAMESPACE,
            &[("name", "text"), ("url", "url"), ("id", "text")],
        ),
        "flash" => (
            EXIF_NAMESPACE,
            &[
                ("Function", "boolean"),
                ("Return", "text"),
                ("RedEyeMode", "boolean"),
                ("Fired", "boolean"),
                ("Mode", "text"),
            ],
        ),
        "oecf/sfr" => (
            EXIF_NAMESPACE,
            &[
                ("Names", "seq text"),
                ("Values", "seq rational"),
                ("Columns", "integer"),
                ("Rows", "integer"),
            ],
        ),
        "cfapattern" => (
            EXIF_NAMESPACE,
            &[
                ("Values", "seq integer"),
                ("Columns", "integer"),
                ("Rows", "integer"),
            ],
        ),
        "devicesettings" => (
            EXIF_NAMESPACE,
            &[
                ("Columns", "integer"),
                ("Settings", "seq text"),
                ("Rows", "integer"),
            ],
        ),
        "dimensions" => (
            XMP_DIMENSIONS_NAMESPACE,
            &[("h", "real"), ("unit", "text"), ("w", "real")],
        ),
        _ => return None,
    })
}

fn structured_xmp_value_matches(
    property: XmpProperty<'_>,
    namespace: &str,
    fields: &[(&str, &str)],
    extension_types: Option<&BTreeMap<String, ExtensionTypeDefinition>>,
) -> bool {
    let Some(fields_in_value) = structured_xmp_fields(property) else {
        return false;
    };
    fields_in_value.into_iter().all(|field| {
        let Some((_, value_type)) = fields
            .iter()
            .find(|(name, _)| field.namespace() == Some(namespace) && field.name() == *name)
        else {
            return false;
        };
        value_type_matches(field, value_type, extension_types)
    })
}

fn extension_xmp_value_matches(
    property: XmpProperty<'_>,
    definition: &ExtensionTypeDefinition,
    extension_types: Option<&BTreeMap<String, ExtensionTypeDefinition>>,
) -> bool {
    let ExtensionTypeDefinition::Structured { namespace, fields } = definition else {
        return property.is_simple();
    };
    let Some(fields_in_value) = structured_xmp_fields(property) else {
        return false;
    };
    fields_in_value.into_iter().all(|field| {
        if field.namespace() != Some(namespace) {
            return false;
        }
        let Some(value_type) = fields.get(field.name()) else {
            return false;
        };
        value_type_matches(field, value_type, extension_types)
    })
}

fn structured_xmp_fields(property: XmpProperty<'_>) -> Option<Vec<XmpProperty<'_>>> {
    if let Some(value) = property.qualified_value() {
        return structured_xmp_fields(value);
    }
    let XmpProperty::Element(node) = property else {
        return None;
    };
    if node.attribute((RDF_NAMESPACE, "parseType")) == Some("Resource") {
        return Some(child_properties(node));
    }
    let mut children = node.children().filter(|child| child.is_element());
    let description = children.next().filter(|child| {
        child.tag_name().namespace() == Some(RDF_NAMESPACE)
            && child.tag_name().name() == "Description"
    })?;
    children
        .next()
        .is_none()
        .then(|| child_properties(description))
}

fn scalar_xmp_value_matches(value: &str, value_type: &str) -> bool {
    match value_type {
        "boolean" => matches!(value, "True" | "False"),
        "integer" => signed_decimal(value, false),
        "real" => signed_decimal(value, true),
        "date" => xmp_iso8601_date(value),
        "gpscoordinate" => gps_coordinate(value),
        "xpath" => XPathFactory::new()
            .build(value)
            .is_ok_and(|expression| expression.is_some()),
        "mimetype" => value.split_once('/').is_some_and(|(left, right)| {
            !left.is_empty()
                && !right.is_empty()
                && left.bytes().all(mime_type_byte)
                && right.bytes().all(mime_type_byte)
        }),
        // Pinned URI and URL validators only require an XMP simple node.
        // XPath syntax is compiled above with an XPath 1.0 parser.
        _ => true,
    }
}

fn gps_coordinate(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 9
        && bytes[0..2].iter().all(u8::is_ascii_digit)
        && bytes[2] == b','
        && bytes[3..5].iter().all(u8::is_ascii_digit)
        && matches!(bytes[5], b',' | b'.')
        && bytes[6..8].iter().all(u8::is_ascii_digit)
        && matches!(bytes[8], b'N' | b'S' | b'E' | b'W')
}

fn xmp_iso8601_date(value: &str) -> bool {
    ParsedDate::from_xmp(value).is_some()
}

fn signed_decimal(value: &str, allow_decimal_point: bool) -> bool {
    let value = value.strip_prefix(['+', '-']).unwrap_or(value);
    let mut parts = value.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if parts.next().is_some() || (!allow_decimal_point && fraction.is_some()) {
        return false;
    }
    let whole_valid = !whole.is_empty() && whole.bytes().all(|byte| byte.is_ascii_digit());
    let fraction_valid = fraction.is_some_and(|fraction| {
        !fraction.is_empty() && fraction.bytes().all(|byte| byte.is_ascii_digit())
    });
    whole_valid || (allow_decimal_point && fraction_valid)
}

fn mime_type_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'+' | b'.')
}

fn inspect_extension_xmp_value_types(
    rdf: Node<'_, '_>,
    xml: &str,
    definitions: &ExtensionSchemaDefinitions,
) -> BTreeSet<String> {
    let mut invalid = BTreeSet::new();
    for property in xmp_properties(rdf, xml) {
        let Some(namespace) = property.namespace() else {
            continue;
        };
        if predefined_xmp2004_namespace(namespace) {
            continue;
        }
        let Some(value_type) = definitions
            .properties
            .get(&(namespace.to_owned(), property.name().to_owned()))
        else {
            invalid.insert(format!("{{{namespace}}}{} (undefined)", property.name()));
            continue;
        };
        if !value_type_matches(property, value_type, definitions.types.get(namespace)) {
            invalid.insert(format!("{{{namespace}}}{} ({value_type})", property.name()));
        }
    }
    invalid
}

fn inspect_undefined_extension_xmp_properties(
    rdf: Node<'_, '_>,
    xml: &str,
    definitions: &ExtensionSchemaDefinitions,
) -> BTreeSet<String> {
    let mut undefined = BTreeSet::new();
    for property in xmp_properties(rdf, xml) {
        let Some(namespace) = property.namespace() else {
            continue;
        };
        if !predefined_xmp2004_namespace(namespace)
            && !definitions
                .properties
                .contains_key(&(namespace.to_owned(), property.name().to_owned()))
        {
            undefined.insert(format!("{{{namespace}}}{}", property.name()));
        }
    }
    undefined
}

fn extension_schema_property_definitions(rdf: Node<'_, '_>) -> ExtensionSchemaDefinitions {
    let mut definitions = ExtensionSchemaDefinitions::default();
    for container in property_nodes(rdf, PDFA_EXTENSION_NAMESPACE, "schemas") {
        for schema in XmpProperty::Element(container).array_items() {
            let Some(namespace) = field_value(schema, PDFA_SCHEMA_NAMESPACE, "namespaceURI") else {
                continue;
            };
            let Some(property_node) = fields(schema, PDFA_SCHEMA_NAMESPACE, "property")
                .into_iter()
                .rev()
                .find(|property| property.array_kind().is_some())
            else {
                continue;
            };

            let mut extension_types = BTreeMap::new();
            if let Some(value_type_node) = fields(schema, PDFA_SCHEMA_NAMESPACE, "valueType")
                .into_iter()
                .rev()
                .find(|value_type| value_type.array_kind().is_some())
            {
                for definition in value_type_node.array_items() {
                    let Some(name) = field_value(definition, PDFA_TYPE_NAMESPACE, "type") else {
                        continue;
                    };
                    let fields_node = fields(definition, PDFA_TYPE_NAMESPACE, "field")
                        .into_iter()
                        .rev()
                        .find(|field| field.array_kind().is_some());
                    let mut field_definitions = BTreeMap::new();
                    if let Some(fields_node) = fields_node {
                        for field in fields_node.array_items() {
                            let (Some(name), Some(value_type)) = (
                                field_value(field, PDFA_FIELD_NAMESPACE, "name"),
                                field_value(field, PDFA_FIELD_NAMESPACE, "valueType"),
                            ) else {
                                continue;
                            };
                            field_definitions.insert(name.to_owned(), value_type.to_owned());
                        }
                    }
                    let definition = if field_definitions.is_empty() {
                        ExtensionTypeDefinition::Simple
                    } else if let Some(type_namespace) =
                        field_value(definition, PDFA_TYPE_NAMESPACE, "namespaceURI")
                    {
                        ExtensionTypeDefinition::Structured {
                            namespace: type_namespace.to_owned(),
                            fields: field_definitions,
                        }
                    } else {
                        continue;
                    };
                    extension_types.insert(xmp_type_key(name), definition);
                }
            }

            let mut schema_properties = BTreeMap::new();
            let known_types = extension_types
                .keys()
                .cloned()
                .chain(base_xmp_types())
                .collect::<BTreeSet<_>>();
            for property in property_node.array_items() {
                let (Some(name), Some(value_type)) = (
                    field_value(property, PDFA_PROPERTY_NAMESPACE, "name"),
                    field_value(property, PDFA_PROPERTY_NAMESPACE, "valueType"),
                ) else {
                    continue;
                };
                if xmp_type_is_known(value_type, &known_types) {
                    schema_properties
                        .entry((namespace.to_owned(), name.to_owned()))
                        .or_insert_with(|| value_type.to_owned());
                }
            }

            definitions
                .properties
                .retain(|(property_namespace, _), _| property_namespace != namespace);
            definitions.properties.extend(schema_properties);
            definitions
                .types
                .insert(namespace.to_owned(), extension_types);
        }
    }
    definitions
}

#[derive(Default)]
struct ExtensionSchemaDefinitions {
    properties: BTreeMap<(String, String), String>,
    types: BTreeMap<String, BTreeMap<String, ExtensionTypeDefinition>>,
}

enum ExtensionTypeDefinition {
    Simple,
    Structured {
        namespace: String,
        fields: BTreeMap<String, String>,
    },
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
    fn qualified_value(self) -> Option<Self> {
        let Self::Element(node) = self else {
            return None;
        };
        if let Some(value) = node.children().find(|child| {
            child.is_element()
                && child.tag_name().namespace() == Some(RDF_NAMESPACE)
                && child.tag_name().name() == "value"
        }) {
            return Some(Self::Element(value));
        }
        let mut node_elements = node.children().filter(|child| child.is_element());
        let value_parent = node_elements.next()?;
        node_elements.next().is_none().then_some(())?;
        value_parent
            .children()
            .find(|child| {
                child.is_element()
                    && child.tag_name().namespace() == Some(RDF_NAMESPACE)
                    && child.tag_name().name() == "value"
            })
            .map(Self::Element)
    }

    fn namespace(self) -> Option<&'a str> {
        match self {
            Self::Element(node) => node.tag_name().namespace().map(normalized_xmp_namespace),
            Self::Attribute(attribute) => attribute.namespace().map(normalized_xmp_namespace),
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
                if let Some(value) = self.qualified_value() {
                    return value.is_simple();
                }
                node.attribute((RDF_NAMESPACE, "parseType")) != Some("Resource")
                    && !node.children().any(|child| child.is_element())
            }
        }
    }

    fn value(self) -> Option<&'a str> {
        match self {
            Self::Attribute(attribute) => Some(attribute.value()),
            Self::Element(node) => {
                if let Some(value) = self.qualified_value() {
                    return value.value();
                }
                node.attribute((RDF_NAMESPACE, "value"))
                    .or_else(|| node.attribute((RDF_NAMESPACE, "resource")))
                    .or_else(|| node.text())
                    .or_else(|| (!node.children().any(|child| child.is_element())).then_some(""))
            }
        }
    }

    fn array_kind(self) -> Option<ArrayKind> {
        let Self::Element(node) = self else {
            return None;
        };
        if let Some(value) = self.qualified_value() {
            return value.array_kind();
        }
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
        if let Some(value) = self.qualified_value() {
            return value.array_items();
        }
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

fn inspect_extension_schemas(rdf: Node<'_, '_>, xml: &str) -> BTreeSet<u8> {
    let mut failed = BTreeSet::new();
    for container in top_level_properties(rdf, xml).into_iter().filter(|property| {
        property.namespace() == Some(PDFA_EXTENSION_NAMESPACE) && property.name() == "schemas"
    }) {
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

fn inspect_identification_prefixes(rdf: Node<'_, '_>, xml: &str) -> BTreeSet<u8> {
    [("part", 4), ("conformance", 5), ("amd", 6)]
        .into_iter()
        .filter_map(|(name, test)| {
            first_document_property(rdf, xml, PDFA_ID_NAMESPACE, name)
                .and_then(|property| property.prefix(xml))
                .is_some_and(|prefix| prefix != "pdfaid")
                .then_some(test)
        })
        .collect()
}

fn first_document_property<'a>(
    rdf: Node<'a, 'a>,
    xml: &str,
    namespace: &str,
    name: &str,
) -> Option<XmpProperty<'a>> {
    top_level_properties(rdf, xml)
        .into_iter()
        .find(|property| property.namespace() == Some(namespace) && property.name() == name)
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
    if !optional_sequence_is_valid(value_type, "pdfaSchema", xml, value_type_item_is_valid) {
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
        .is_some_and(|value| xmp_type_is_known(value, known_types))
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
            .is_some_and(|value| xmp_type_is_known(value, known_types))
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
    let mut known = base_xmp_types().collect::<BTreeSet<_>>();
    for value_type in fields(definition, PDFA_SCHEMA_NAMESPACE, "valueType") {
        for item in value_type.array_items() {
            if let Some(value) = field_value(item, PDFA_TYPE_NAMESPACE, "type") {
                known.insert(xmp_type_key(value));
            }
        }
    }
    known
}

fn base_xmp_types() -> impl Iterator<Item = String> {
    [
        "agentname",
        "any",
        "boolean",
        "cfapattern",
        "date",
        "devicesettings",
        "dimensions",
        "flash",
        "gpscoordinate",
        "integer",
        "job",
        "lang alt",
        "locale",
        "mimetype",
        "oecf/sfr",
        "propername",
        "rational",
        "real",
        "renditionclass",
        "resourceevent",
        "resourceref",
        "text",
        "thumbnail",
        "uri",
        "url",
        "version",
        "xpath",
    ]
    .into_iter()
    .map(str::to_owned)
}

fn xmp_type_is_known(value_type: &str, known_types: &BTreeSet<String>) -> bool {
    let mut value_type = xmp_type_key(value_type);
    loop {
        if known_types.contains(&value_type) {
            return true;
        }
        let Some((_, item_type)) = value_type.split_once(' ') else {
            return false;
        };
        if !matches!(value_type.split_once(' '), Some(("bag" | "seq" | "alt", _))) {
            return false;
        }
        value_type = item_type.to_owned();
    }
}

fn select_xmp_root<'a>(document: &'a Document<'a>) -> Option<(Node<'a, 'a>, Option<&'a str>)> {
    let mut packet_header = None;
    find_xmp_root(document.root(), false, &mut packet_header).map(|rdf| (rdf, packet_header))
}

fn find_xmp_root<'a>(
    parent: Node<'a, 'a>,
    xmpmeta_required: bool,
    packet_header: &mut Option<&'a str>,
) -> Option<Node<'a, 'a>> {
    for child in parent.children() {
        if let Some(instruction) = child.pi() {
            if instruction.target == "xpacket" {
                *packet_header = instruction.value;
            }
            continue;
        }
        if child.is_text() {
            continue;
        }
        if child.is_element()
            && child.tag_name().namespace() == Some("adobe:ns:meta/")
            && matches!(child.tag_name().name(), "xmpmeta" | "xapmeta")
        {
            return find_xmp_root(child, false, packet_header);
        } else if !xmpmeta_required
            && child.is_element()
            && child.tag_name().namespace() == Some(RDF_NAMESPACE)
            && child.tag_name().name() == "RDF"
        {
            return Some(child);
        } else if let Some(rdf) = find_xmp_root(child, xmpmeta_required, packet_header) {
            return Some(rdf);
        }
    }
    None
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

fn normalized_xmp_namespace(namespace: &str) -> &str {
    if namespace == DC_DEPRECATED_NAMESPACE {
        DC_NAMESPACE
    } else {
        namespace
    }
}

/// `ParseRDF.isWhitespaceNode` uses `Character.isWhitespace(char)`, whose
/// definition deliberately excludes the three non-breaking space characters
/// that Rust's Unicode `trim`/`is_whitespace` includes.
fn java_text_is_whitespace(text: &str) -> bool {
    text.chars().all(|character| {
        matches!(
            character,
            '\u{0009}'..='\u{000d}'
                | '\u{001c}'..='\u{001f}'
                | '\u{0020}'
                | '\u{1680}'
                | '\u{2000}'..='\u{2006}'
                | '\u{2008}'..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{205f}'
                | '\u{3000}'
        )
    })
}

/// Java `String.trim()` removes only code units at or below U+0020.
fn java_string_trim(value: &str) -> &str {
    value.trim_matches(|character| character <= '\u{0020}')
}

fn source_element_qname<'a>(node: Node<'_, '_>, xml: &'a str) -> Option<&'a str> {
    let source = xml.get(node.range())?;
    source
        .strip_prefix('<')?
        .split(|character: char| {
            character.is_ascii_whitespace() || matches!(character, '/' | '>')
        })
        .next()
}

fn cdata_positions(xml: &str, range: std::ops::Range<usize>) -> Vec<usize> {
    let bytes = xml.as_bytes();
    let mut positions = Vec::new();
    let mut position = range.start;
    while position < range.end {
        let Some(relative) = bytes[position..range.end]
            .iter()
            .position(|byte| *byte == b'<')
        else {
            break;
        };
        position += relative;
        let tail = &bytes[position..range.end];
        if tail.starts_with(b"<!--") {
            match bytes[position + 4..range.end]
                .windows(3)
                .position(|window| window == b"-->")
            {
                Some(relative) => position += 4 + relative + 3,
                None => break,
            }
        } else if tail.starts_with(b"<![CDATA[") {
            positions.push(position);
            match bytes[position + 9..range.end]
                .windows(3)
                .position(|window| window == b"]]>")
            {
                Some(relative) => position += 9 + relative + 3,
                None => break,
            }
        } else {
            match find_xml_tag_end(bytes, position + 1) {
                Ok((end, _)) => position = end,
                Err(_) => break,
            }
        }
    }
    positions
}

fn is_ignored_ix_changes_property(property: Node<'_, '_>, xml: &str) -> bool {
    if source_element_qname(property, xml) != Some("iX:changes")
        || property.attributes().any(|attribute| {
            attribute.namespace() != Some(XML_NAMESPACE) || attribute.name() != "lang"
        })
    {
        return false;
    }
    property
        .children()
        .any(|child| !child.is_text() && !child.is_comment())
        || !cdata_positions(xml, property.range()).is_empty()
}

fn validate_cdata_usage(rdf: Node<'_, '_>, xml: &str) -> Result<(), String> {
    let ignored = top_level_descriptions(rdf)
        .flat_map(|description| description.children())
        .filter(|property| property.is_element() && is_ignored_ix_changes_property(*property, xml))
        .map(|property| property.range())
        .collect::<Vec<_>>();
    if cdata_positions(xml, rdf.range())
        .into_iter()
        .any(|position| !ignored.iter().any(|range| range.contains(&position)))
    {
        Err("CDATA nodes are not accepted by the XMP 2004 RDF parser".to_owned())
    } else {
        Ok(())
    }
}

fn top_level_descriptions<'a>(rdf: Node<'a, 'a>) -> impl Iterator<Item = Node<'a, 'a>> {
    rdf.children().filter(|node| {
        node.is_element()
            && node.tag_name().namespace() == Some(RDF_NAMESPACE)
            && node.tag_name().name() == "Description"
    })
}

fn top_level_properties<'a>(rdf: Node<'a, 'a>, xml: &str) -> Vec<XmpProperty<'a>> {
    top_level_descriptions(rdf)
        .flat_map(|description| {
            description
                .attributes()
                .filter(move |attribute| {
                    !matches!(
                        rdf_attribute_term(description, *attribute),
                        RdfTerm::Id | RdfTerm::NodeId | RdfTerm::About
                    )
                })
                .map(XmpProperty::Attribute)
                .chain(
                    description
                        .children()
                        .filter(|child| child.is_element())
                        .filter(|child| !is_ignored_ix_changes_property(*child, xml))
                        .map(XmpProperty::Element),
                )
        })
        .collect()
}

fn xmp_properties<'a>(rdf: Node<'a, 'a>, xml: &str) -> Vec<XmpProperty<'a>> {
    top_level_properties(rdf, xml)
        .into_iter()
        .filter(|property| {
            property.namespace() != Some(PDFA_EXTENSION_NAMESPACE) || property.name() != "schemas"
        })
        .collect()
}

fn contains_namespace_property(rdf: Node<'_, '_>, xml: &str, namespace: &str) -> bool {
    xmp_properties(rdf, xml)
        .into_iter()
        .any(|property| property.namespace() == Some(namespace))
}

fn property_nodes<'a>(rdf: Node<'a, 'a>, namespace: &str, local_name: &str) -> Vec<Node<'a, 'a>> {
    top_level_descriptions(rdf)
        .flat_map(|description| description.children())
        .filter(|property| {
            property.is_element()
                && property
                    .tag_name()
                    .namespace()
                    .map(normalized_xmp_namespace)
                    == Some(namespace)
                && property.tag_name().name() == local_name
        })
        .collect()
}

fn property_values(
    rdf: Node<'_, '_>,
    xml: &str,
    namespace: &str,
    local_name: &str,
) -> Vec<String> {
    let mut values = Vec::new();
    for property in xmp_properties(rdf, xml) {
        if property.namespace() == Some(namespace)
            && property.name() == local_name
            && let Some(value) = property.value()
        {
            values.push(value.to_owned());
        }
    }
    values
}

fn localized_text_values(rdf: Node<'_, '_>, namespace: &str, local_name: &str) -> Vec<String> {
    property_nodes(rdf, namespace, local_name)
        .into_iter()
        .filter_map(localized_text_value)
        .collect()
}

fn localized_text_value(property: Node<'_, '_>) -> Option<String> {
    let property = XmpProperty::Element(property);
    (property.array_kind() == Some(ArrayKind::Alt)).then_some(())?;
    let items = property.array_items();
    if items.iter().any(|item| {
        item.tag_name().namespace() != Some(RDF_NAMESPACE)
            || item.tag_name().name() != "li"
            || !XmpProperty::Element(*item).is_simple()
            || item.attribute((XML_NAMESPACE, "lang")).is_none()
    }) {
        return None;
    }
    items
        .iter()
        .find(|item| item.attribute((XML_NAMESPACE, "lang")) == Some("x-default"))
        .or_else(|| {
            items.iter().find(|item| {
                item.attribute((XML_NAMESPACE, "lang"))
                    .is_some_and(|language| language.starts_with('x'))
            })
        })
        .or_else(|| items.first())
        .and_then(|item| XmpProperty::Element(*item).value())
        .map(str::to_owned)
}

fn ordered_array_values(property: Node<'_, '_>) -> Option<Vec<String>> {
    let property = XmpProperty::Element(property);
    matches!(property.array_kind(), Some(ArrayKind::Seq | ArrayKind::Alt)).then_some(())?;
    property
        .array_items()
        .into_iter()
        .map(|item| {
            let item = XmpProperty::Element(item);
            item.is_simple()
                .then(|| item.value().map(str::to_owned))
                .flatten()
        })
        .collect()
}

fn decode_xml(bytes: &[u8]) -> Result<String, String> {
    let xml = if bytes.starts_with(&[0, 0, 0xFE, 0xFF])
        || bytes.len() >= 4 && bytes[0] == 0 && bytes[1] == 0 && bytes[2] == 0
    {
        decode_utf32(bytes.strip_prefix(&[0, 0, 0xFE, 0xFF]).unwrap_or(bytes), true)
    } else if bytes.starts_with(&[0xFF, 0xFE, 0, 0])
        || bytes.len() >= 4 && bytes[0] != 0 && bytes[1] == 0 && bytes[2] == 0
    {
        decode_utf32(bytes.strip_prefix(&[0xFF, 0xFE, 0, 0]).unwrap_or(bytes), false)
    } else if bytes.starts_with(&[0xFE, 0xFF])
        || bytes.len() >= 2 && bytes[0] == 0 && bytes[1] != 0
    {
        decode_utf16_lossy(
            bytes.strip_prefix(&[0xFE, 0xFF]).unwrap_or(bytes),
            u16::from_be_bytes,
        )
    } else if bytes.starts_with(&[0xFF, 0xFE])
        || bytes.len() >= 2 && bytes[0] != 0 && bytes[1] == 0
    {
        decode_utf16_lossy(
            bytes.strip_prefix(&[0xFF, 0xFE]).unwrap_or(bytes),
            u16::from_le_bytes,
        )
    } else {
        let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
        if let Ok(xml) = std::str::from_utf8(bytes) {
            return Ok(xml.to_owned());
        }
        let repaired = repair_latin1_utf8(bytes);
        let mut xml = String::from_utf8_lossy(&repaired).into_owned();
        repair_xml_controls(&mut xml);
        return Ok(xml);
    };
    Ok(xml)
}

fn decode_utf16_lossy(bytes: &[u8], convert: fn([u8; 2]) -> u16) -> String {
    let mut units = bytes
        .chunks_exact(2)
        .map(|pair| convert([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    if !bytes.len().is_multiple_of(2) {
        units.push(0xFFFD);
    }
    String::from_utf16_lossy(&units)
}

fn decode_utf32(bytes: &[u8], big_endian: bool) -> String {
    let mut result = String::new();
    let mut chunks = bytes.chunks_exact(4);
    for bytes in &mut chunks {
        let bytes = [bytes[0], bytes[1], bytes[2], bytes[3]];
        let value = if big_endian {
            u32::from_be_bytes(bytes)
        } else {
            u32::from_le_bytes(bytes)
        };
        result.push(char::from_u32(value).unwrap_or('\u{FFFD}'));
    }
    if !chunks.remainder().is_empty() {
        result.push('\u{FFFD}');
    }
    result
}

fn repair_latin1_utf8(bytes: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(bytes.len());
    let mut position = 0;
    while position < bytes.len() {
        let byte = bytes[position];
        if byte < 127 {
            result.push(byte);
            position += 1;
            continue;
        }
        if byte >= 192 {
            let continuation_count = byte.leading_ones().saturating_sub(1) as usize;
            let end = position.saturating_add(continuation_count + 1);
            if continuation_count > 0
                && end <= bytes.len()
                && bytes[position + 1..end]
                    .iter()
                    .all(|byte| byte & 0xC0 == 0x80)
            {
                result.extend_from_slice(&bytes[position..end]);
                position = end;
                continue;
            }
        }
        push_cp1252_as_utf8(&mut result, byte);
        position += 1;
    }
    result
}

fn push_cp1252_as_utf8(result: &mut Vec<u8>, byte: u8) {
    let character = match byte {
        0x80 => '\u{20AC}',
        0x81 | 0x8D | 0x8F | 0x90 | 0x9D => ' ',
        0x82 => '\u{201A}',
        0x83 => '\u{0192}',
        0x84 => '\u{201E}',
        0x85 => '\u{2026}',
        0x86 => '\u{2020}',
        0x87 => '\u{2021}',
        0x88 => '\u{02C6}',
        0x89 => '\u{2030}',
        0x8A => '\u{0160}',
        0x8B => '\u{2039}',
        0x8C => '\u{0152}',
        0x8E => '\u{017D}',
        0x91 => '\u{2018}',
        0x92 => '\u{2019}',
        0x93 => '\u{201C}',
        0x94 => '\u{201D}',
        0x95 => '\u{2022}',
        0x96 => '\u{2013}',
        0x97 => '\u{2014}',
        0x98 => '\u{02DC}',
        0x99 => '\u{2122}',
        0x9A => '\u{0161}',
        0x9B => '\u{203A}',
        0x9C => '\u{0153}',
        0x9E => '\u{017E}',
        0x9F => '\u{0178}',
        _ => char::from_u32(u32::from(byte)).expect("byte is a Unicode scalar"),
    };
    let mut encoded = [0; 4];
    result.extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
}

fn repair_xml_controls(xml: &mut String) {
    let characters = xml.chars().collect::<Vec<_>>();
    let mut repaired = String::with_capacity(xml.len());
    let mut position = 0;
    while position < characters.len() {
        if characters[position] == '&'
            && characters.get(position + 1) == Some(&'#')
            && let Some((end, value)) = numeric_character_reference(&characters, position)
            && is_xmp_ascii_control(value)
        {
            repaired.push(' ');
            position = end;
            continue;
        }
        let character = characters[position];
        repaired.push(if is_xmp_ascii_control(character as u32) {
            ' '
        } else {
            character
        });
        position += 1;
    }
    *xml = repaired;
}

fn numeric_character_reference(characters: &[char], start: usize) -> Option<(usize, u32)> {
    let mut position = start + 2;
    let radix = if characters.get(position) == Some(&'x') {
        position += 1;
        16
    } else {
        10
    };
    let digit_start = position;
    let maximum_digits = if radix == 16 { 4 } else { 5 };
    let mut value = 0_u32;
    while position < characters.len() && position - digit_start < maximum_digits {
        let Some(digit) = characters[position].to_digit(radix) else {
            break;
        };
        value = value * radix + digit;
        position += 1;
    }
    (position > digit_start && characters.get(position) == Some(&';'))
        .then_some((position + 1, value))
}

fn is_xmp_ascii_control(value: u32) -> bool {
    value <= 31 && !matches!(value, 9 | 10 | 13) || value == 127
}

pub(crate) fn dates_equivalent(pdf: &str, xmp: &str) -> bool {
    let Some(pdf) = ParsedDate::from_pdf(pdf) else {
        return false;
    };
    let Some(xmp) = ParsedDate::from_xmp(xmp) else {
        return false;
    };
    pdf.instant_millis()
        .zip(xmp.instant_millis())
        .is_some_and(|(pdf, xmp)| pdf == xmp)
}

#[derive(Clone, Copy, Debug)]
struct ParsedDate {
    year: i32,
    month_index: i32,
    day: i32,
    hour: i32,
    minute: i32,
    second: i32,
    millisecond: i32,
    zone: DateZone,
}

#[derive(Clone, Copy, Debug)]
enum DateZone {
    Fixed(i32),
    Local,
}

impl ParsedDate {
    fn from_pdf(value: &str) -> Option<Self> {
        let mut value = value.strip_prefix("D:")?;
        if value.ends_with('\'') {
            if value.ends_with("''") {
                return None;
            }
            value = &value[..value.len() - 1];
        }
        if value.len() < 4 || value.len() > 20 {
            return None;
        }
        let year = digits(value, 0, 4)? as i32;
        let month_index = pdf_optional_pair(value, 4, 1)? - 1;
        let day = pdf_optional_pair(value, 6, 1)?;
        let hour = pdf_optional_pair(value, 8, 0)?;
        let minute = pdf_optional_pair(value, 10, 0)?;
        let second = pdf_optional_pair(value, 12, 0)?;
        let timezone = value.get(value.len().min(14)..)?;
        let offset_minutes = parse_pdf_offset(timezone)?;
        Some(Self {
            year,
            month_index,
            day,
            hour,
            minute,
            second,
            millisecond: 0,
            zone: DateZone::Fixed(offset_minutes),
        })
    }

    fn from_xmp(value: &str) -> Option<Self> {
        if value.is_empty() {
            return Some(Self::empty_xmp());
        }
        let bytes = value.as_bytes();
        let mut position = usize::from(bytes.first() == Some(&b'-'));
        let negative = position == 1;
        let year = gather_xmp_integer(bytes, &mut position, 9_999)?;
        let year = if negative { year.abs() } else { year };
        let mut result = Self {
            year,
            ..Self::empty_xmp()
        };
        if position == bytes.len() {
            return Some(result);
        }
        take_byte(bytes, &mut position, b'-')?;
        result.month_index = gather_xmp_integer(bytes, &mut position, 12)?.clamp(1, 12) - 1;
        if position == bytes.len() {
            return Some(result);
        }
        take_byte(bytes, &mut position, b'-')?;
        result.day = gather_xmp_integer(bytes, &mut position, 31)?.clamp(1, 31);
        if position == bytes.len() {
            return Some(result);
        }
        take_byte(bytes, &mut position, b'T')?;
        result.hour = gather_xmp_integer(bytes, &mut position, 23)?.clamp(0, 23);
        if position == bytes.len() {
            return Some(result);
        }
        if bytes[position] == b':' {
            position += 1;
            result.minute = gather_xmp_integer(bytes, &mut position, 59)?.clamp(0, 59);
            if position == bytes.len() {
                return Some(result);
            }
            if bytes[position] == b':' {
                position += 1;
                result.second = gather_xmp_integer(bytes, &mut position, 59)?.clamp(0, 59);
                if position < bytes.len() && bytes[position] == b'.' {
                    position += 1;
                    let fraction_start = position;
                    gather_xmp_integer(bytes, &mut position, 999_999_999)?;
                    let fraction = &bytes[fraction_start..position];
                    result.millisecond = fraction
                        .iter()
                        .take(3)
                        .fold(0, |value, digit| value * 10 + i32::from(digit - b'0'))
                        * 10_i32.pow(3_u32.saturating_sub(fraction.len().min(3) as u32));
                }
            } else if !matches!(bytes[position], b'Z' | b'+' | b'-') {
                return None;
            }
        } else if !matches!(bytes[position], b'Z' | b'+' | b'-') {
            return None;
        }
        if position == bytes.len() {
            return Some(result);
        }
        if bytes[position] == b'Z' {
            position += 1;
            result.zone = DateZone::Fixed(0);
        } else {
            let sign = if bytes[position] == b'-' { -1 } else { 1 };
            position += 1;
            let zone_hour = gather_xmp_integer(bytes, &mut position, 23)?.clamp(0, 23);
            let mut zone_minute = 0;
            if position < bytes.len() {
                take_byte(bytes, &mut position, b':')?;
                zone_minute = gather_xmp_integer(bytes, &mut position, 59)?.clamp(0, 59);
            }
            result.zone = DateZone::Fixed(sign * (zone_hour * 60 + zone_minute));
        }
        (position == bytes.len()).then_some(result)
    }

    fn empty_xmp() -> Self {
        Some(Self {
            year: 0,
            month_index: -1,
            day: 0,
            hour: 0,
            minute: 0,
            second: 0,
            millisecond: 0,
            zone: DateZone::Local,
        })
        .expect("date literal")
    }

    fn instant_millis(self) -> Option<i64> {
        let normalized_year = self.year + self.month_index.div_euclid(12);
        let normalized_month = self.month_index.rem_euclid(12) as u32 + 1;
        let naive = NaiveDate::from_ymd_opt(normalized_year, normalized_month, 1)?
            .and_hms_opt(0, 0, 0)?
            .checked_add_signed(Duration::days(i64::from(self.day - 1)))?
            .checked_add_signed(Duration::seconds(
                i64::from(self.hour) * 3_600 + i64::from(self.minute) * 60 + i64::from(self.second),
            ))?
            .checked_add_signed(Duration::milliseconds(i64::from(self.millisecond)))?;
        let local_millis = naive.and_utc().timestamp_millis();
        match self.zone {
            DateZone::Fixed(offset) => Some(local_millis - i64::from(offset) * 60_000),
            DateZone::Local => match Local.from_local_datetime(&naive) {
                LocalResult::Single(value) => Some(value.timestamp_millis()),
                LocalResult::Ambiguous(left, right) => {
                    Some(left.timestamp_millis().max(right.timestamp_millis()))
                }
                LocalResult::None => local_offset_before(naive)
                    .map(|offset| local_millis - i64::from(offset) * 1_000),
            },
        }
    }
}

fn digits(value: &str, start: usize, length: usize) -> Option<u32> {
    value.get(start..start + length)?.parse().ok()
}

fn pdf_optional_pair(value: &str, start: usize, default: i32) -> Option<i32> {
    if value.len() <= start {
        Some(default)
    } else {
        digits(value, start, 2).map(|value| value as i32)
    }
}

fn parse_pdf_offset(value: &str) -> Option<i32> {
    if value.is_empty() {
        return Some(0);
    }
    let sign = match value.as_bytes()[0] {
        b'Z' => 0,
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    if value.len() == 1 {
        return Some(0);
    }
    if sign == 0 || !matches!(value.len(), 3 | 4 | 6) {
        return None;
    }
    let hours = digits(value, 1, 2)? as i32;
    let minutes = if value.len() > 3 {
        if value.as_bytes().get(3) != Some(&b'\'') || value.len() != 6 {
            return None;
        }
        digits(value, 4, 2)? as i32
    } else {
        0
    };
    if hours <= 23 && minutes <= 59 {
        Some(sign * (hours * 60 + minutes))
    } else {
        // java.util.TimeZone silently falls back to GMT for malformed custom
        // GMT offsets after the lexical PDF date checks have succeeded.
        Some(0)
    }
}

fn gather_xmp_integer(bytes: &[u8], position: &mut usize, maximum: i32) -> Option<i32> {
    let start = *position;
    let mut value = 0_i64;
    while bytes.get(*position).is_some_and(u8::is_ascii_digit) {
        value = (value * 10 + i64::from(bytes[*position] - b'0')).min(i64::from(maximum));
        *position += 1;
    }
    (*position > start).then_some(value as i32)
}

fn take_byte(bytes: &[u8], position: &mut usize, expected: u8) -> Option<()> {
    (bytes.get(*position) == Some(&expected)).then(|| *position += 1)
}

fn local_offset_before(naive: chrono::NaiveDateTime) -> Option<i32> {
    (1..=180).find_map(|minutes| {
        let candidate = naive.checked_sub_signed(Duration::minutes(minutes))?;
        match Local.from_local_datetime(&candidate) {
            LocalResult::Single(value) => Some(value.offset().local_minus_utc()),
            LocalResult::Ambiguous(left, right) => Some(
                left.offset()
                    .local_minus_utc()
                    .min(right.offset().local_minus_utc()),
            ),
            LocalResult::None => None,
        }
    })
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
    fn detects_undefined_properties_in_predefined_xmp2004_schemas() {
        let xmp = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:xmp="http://ns.adobe.com/xap/1.0/"><rdf:Description xmp:Unknown="x"/>
          </rdf:RDF>"#;
        let parsed = parse_xmp(xmp).expect("valid XMP");
        assert_eq!(
            parsed.invalid_predefined_xmp_properties,
            BTreeSet::from(["{http://ns.adobe.com/xap/1.0/}Unknown".to_owned()])
        );
    }

    #[test]
    fn detects_incompatible_predefined_xmp2004_value_shapes() {
        let xmp = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:dc="http://purl.org/dc/elements/1.1/"><rdf:Description>
          <dc:title>not a language alternative</dc:title></rdf:Description></rdf:RDF>"#;
        let parsed = parse_xmp(xmp).expect("valid XMP");
        assert_eq!(
            parsed.invalid_predefined_xmp_value_types,
            BTreeSet::from(["{http://purl.org/dc/elements/1.1/}title (lang alt)".to_owned()])
        );
    }

    #[test]
    fn accepts_a_lang_alt_when_any_item_has_a_language() {
        let xmp = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:dc="http://purl.org/dc/elements/1.1/"><rdf:Description><dc:rights>
          <rdf:Alt><rdf:li xml:lang="x-default">Rights</rdf:li><rdf:li>Unqualified</rdf:li>
          </rdf:Alt></dc:rights></rdf:Description></rdf:RDF>"#;
        let parsed = parse_xmp(xmp).expect("valid XMP");
        assert!(parsed.invalid_predefined_xmp_value_types.is_empty());
    }

    #[test]
    fn maps_the_deprecated_dc_namespace_to_the_xmp_2004_dc_schema() {
        let xmp = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:dc="http://purl.org/dc/1.1/"><rdf:Description><dc:title>
          <rdf:Alt><rdf:li xml:lang="x-default">Title</rdf:li></rdf:Alt>
          </dc:title></rdf:Description></rdf:RDF>"#;
        let parsed = parse_xmp(xmp).expect("valid XMP");
        assert_eq!(parsed.title_x_default, ["Title"]);
        assert!(parsed.invalid_predefined_xmp_properties.is_empty());
        assert!(parsed.undefined_extension_xmp_properties.is_empty());
    }

    #[test]
    fn normalizes_qualified_simple_and_array_properties() {
        let xmp = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:pdf="http://ns.adobe.com/pdf/1.3/"
          xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:q="urn:qualifier">
          <rdf:Description>
            <pdf:Keywords rdf:parseType="Resource"><rdf:value>key</rdf:value><q:kind>one</q:kind></pdf:Keywords>
            <dc:title><rdf:Description><rdf:value><rdf:Alt>
              <rdf:li xml:lang="x-default">Title</rdf:li>
            </rdf:Alt></rdf:value><q:kind>two</q:kind></rdf:Description></dc:title>
          </rdf:Description></rdf:RDF>"#;
        let parsed = parse_xmp(xmp).expect("valid qualified XMP");
        assert_eq!(parsed.keywords, ["key"]);
        assert_eq!(parsed.title_x_default, ["Title"]);
        assert!(parsed.invalid_predefined_xmp_value_types.is_empty());
    }

    #[test]
    fn applies_verapdf_scalar_boolean_and_integer_lexical_forms() {
        let xmp = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/" xmlns:xmpRights="http://ns.adobe.com/xap/1.0/rights/">
          <rdf:Description pdfaid:part="1.2" xmpRights:Marked="true"/></rdf:RDF>"#;
        let parsed = parse_xmp(xmp).expect("valid XMP");
        assert_eq!(
            parsed.invalid_predefined_xmp_value_types,
            BTreeSet::from([
                "{http://www.aiim.org/pdfa/ns/id/}part (integer)".to_owned(),
                "{http://ns.adobe.com/xap/1.0/rights/}Marked (boolean)".to_owned(),
            ])
        );
    }

    #[test]
    fn validates_each_predefined_array_item_against_its_declared_type() {
        let xmp = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:exif="http://ns.adobe.com/exif/1.0/"><rdf:Description><exif:ISOSpeedRatings>
          <rdf:Seq><rdf:li>not-an-integer</rdf:li></rdf:Seq></exif:ISOSpeedRatings>
          </rdf:Description></rdf:RDF>"#;
        let parsed = parse_xmp(xmp).expect("valid XMP");
        assert_eq!(
            parsed.invalid_predefined_xmp_value_types,
            BTreeSet::from([
                "{http://ns.adobe.com/exif/1.0/}ISOSpeedRatings (seq integer)".to_owned()
            ])
        );
    }

    #[test]
    fn validates_predefined_structured_fields_against_the_pinned_definitions() {
        let xmp = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:exif="http://ns.adobe.com/exif/1.0/"><rdf:Description><exif:Flash rdf:parseType="Resource">
          <exif:Fired>not-a-boolean</exif:Fired></exif:Flash></rdf:Description></rdf:RDF>"#;
        let parsed = parse_xmp(xmp).expect("valid XMP");
        assert_eq!(
            parsed.invalid_predefined_xmp_value_types,
            BTreeSet::from(["{http://ns.adobe.com/exif/1.0/}Flash (flash)".to_owned()])
        );
    }

    #[test]
    fn accepts_verapdf_reduced_precision_iso8601_dates() {
        for date in [
            "",
            "2026",
            "2026-07",
            "2026-07-29",
            "2026-07-29T12",
            "2026-07-29T12Z",
            "2026-07-29T12:30",
            "2026-07-29T12:30:45.12+02:00",
            "2026-13",
            "2026-07-29T12:30+02",
            "2026-07-29T12:30+0200",
        ] {
            assert!(xmp_iso8601_date(date), "{date}");
        }
        for date in [
            "2026-07-29T12:30:45.",
            "2026-07Z",
            "2026-07-29T12:30:45.1Zx",
        ] {
            assert!(!xmp_iso8601_date(date), "{date}");
        }
    }

    #[test]
    fn requires_custom_description_properties_to_be_declared_by_an_extension_schema() {
        let xmp = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:pdfaExtension="http://www.aiim.org/pdfa/ns/extension/"
          xmlns:pdfaSchema="http://www.aiim.org/pdfa/ns/schema#"
          xmlns:pdfaProperty="http://www.aiim.org/pdfa/ns/property#" xmlns:custom="urn:custom">
          <rdf:Description><pdfaExtension:schemas><rdf:Bag><rdf:li rdf:parseType="Resource">
          <pdfaSchema:namespaceURI>urn:custom</pdfaSchema:namespaceURI><pdfaSchema:property><rdf:Seq><rdf:li rdf:parseType="Resource">
          <pdfaProperty:name>defined</pdfaProperty:name><pdfaProperty:valueType>Text</pdfaProperty:valueType>
          </rdf:li></rdf:Seq></pdfaSchema:property></rdf:li></rdf:Bag></pdfaExtension:schemas></rdf:Description>
          <rdf:Description custom:defined="yes" custom:missing="no"/></rdf:RDF>"#;
        let parsed = parse_xmp(xmp).expect("valid XMP");
        assert_eq!(
            parsed.undefined_extension_xmp_properties,
            BTreeSet::from(["{urn:custom}missing".to_owned()])
        );
        assert_eq!(
            parsed.invalid_extension_xmp_value_types,
            BTreeSet::from(["{urn:custom}missing (undefined)".to_owned()])
        );
        let mismatched_xml = std::str::from_utf8(xmp)
            .expect("test XML is UTF-8")
            .replace("Text", "Lang Alt");
        let mismatched = parse_xmp(mismatched_xml.as_bytes()).expect("valid XMP");
        assert_eq!(
            mismatched.invalid_extension_xmp_value_types,
            BTreeSet::from([
                "{urn:custom}defined (Lang Alt)".to_owned(),
                "{urn:custom}missing (undefined)".to_owned(),
            ])
        );
    }

    #[test]
    fn validates_custom_extension_type_fields_recursively() {
        let xmp = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:pdfaExtension="http://www.aiim.org/pdfa/ns/extension/"
          xmlns:pdfaSchema="http://www.aiim.org/pdfa/ns/schema#"
          xmlns:pdfaProperty="http://www.aiim.org/pdfa/ns/property#"
          xmlns:pdfaType="http://www.aiim.org/pdfa/ns/type#"
          xmlns:pdfaField="http://www.aiim.org/pdfa/ns/field#" xmlns:custom="urn:custom">
          <rdf:Description><pdfaExtension:schemas><rdf:Bag><rdf:li rdf:parseType="Resource">
          <pdfaSchema:namespaceURI>urn:custom</pdfaSchema:namespaceURI>
          <pdfaSchema:property><rdf:Seq><rdf:li rdf:parseType="Resource">
          <pdfaProperty:name>rating</pdfaProperty:name><pdfaProperty:valueType>Rating</pdfaProperty:valueType>
          </rdf:li></rdf:Seq></pdfaSchema:property>
          <pdfaSchema:valueType><rdf:Seq><rdf:li rdf:parseType="Resource">
          <pdfaType:type>Rating</pdfaType:type><pdfaType:namespaceURI>urn:custom</pdfaType:namespaceURI>
          <pdfaType:field><rdf:Seq><rdf:li rdf:parseType="Resource">
          <pdfaField:name>score</pdfaField:name><pdfaField:valueType>Integer</pdfaField:valueType>
          </rdf:li></rdf:Seq></pdfaType:field>
          </rdf:li></rdf:Seq></pdfaSchema:valueType>
          </rdf:li></rdf:Bag></pdfaExtension:schemas></rdf:Description>
          <rdf:Description><custom:rating rdf:parseType="Resource"><custom:score>invalid</custom:score>
          </custom:rating></rdf:Description></rdf:RDF>"#;
        let parsed = parse_xmp(xmp).expect("valid XMP");
        assert_eq!(
            parsed.invalid_extension_xmp_value_types,
            BTreeSet::from(["{urn:custom}rating (Rating)".to_owned()])
        );
    }

    #[test]
    fn rejects_duplicate_identification_values() {
        let xmp = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/">
          <rdf:Description pdfaid:part="1" pdfaid:conformance="B"/>
          <rdf:Description pdfaid:part="2" pdfaid:conformance="A"/>
        </rdf:RDF>"#;
        let error = parse_xmp(xmp).expect_err("duplicate identification must be rejected");
        assert!(error.contains("declared more than once"), "{error}");
    }

    #[test]
    fn selects_only_the_first_rdf_package_found_by_verapdf() {
        let xmp = br#"<wrapper xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/">
          <rdf:RDF><rdf:Description pdfaid:part="1" pdfaid:conformance="B"/></rdf:RDF>
          <rdf:RDF><rdf:Description pdfaid:part="2" pdfaid:conformance="A"/></rdf:RDF>
        </wrapper>"#;
        let parsed = parse_xmp(xmp).expect("first RDF package is valid");
        assert_eq!(parsed.pdfa_parts, ["1"]);
        assert_eq!(parsed.pdfa_conformances, ["B"]);
    }

    #[test]
    fn an_empty_xmpmeta_stops_the_pinned_root_search() {
        let xmp = br#"<wrapper xmlns:x="adobe:ns:meta/"
          xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/">
          <x:xmpmeta/>
          <rdf:RDF><rdf:Description pdfaid:part="1" pdfaid:conformance="B"/></rdf:RDF>
        </wrapper>"#;
        let parsed = parse_xmp(xmp).expect("empty metadata model");
        assert!(!parsed.pdfa_identification_present);
    }

    #[test]
    fn ignores_top_level_resource_form_ix_changes() {
        let xmp = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:iX="http://ns.adobe.com/iX/1.0/"><rdf:Description>
          <iX:changes><rdf:Description><iX:unknown><![CDATA[ignored]]></iX:unknown>
          </rdf:Description></iX:changes></rdf:Description></rdf:RDF>"#;
        let parsed = parse_xmp(xmp).expect("iX changes are discarded by the pinned parser");
        assert!(parsed.undefined_extension_xmp_properties.is_empty());
    }

    #[test]
    fn rejects_cdata_and_non_java_whitespace_in_selected_rdf() {
        let cdata = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:pdf="http://ns.adobe.com/pdf/1.3/"><rdf:Description>
          <pdf:Producer><![CDATA[value]]></pdf:Producer></rdf:Description></rdf:RDF>"#;
        assert!(parse_xmp(cdata).is_err());

        let nbsp = "<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
          <rdf:Description/>\u{a0}<rdf:Description/></rdf:RDF>";
        assert!(parse_xmp(nbsp.as_bytes()).is_err());
    }

    #[test]
    fn treats_unknown_rdf_attributes_as_xmp_properties() {
        let xmp = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
          <rdf:Description rdf:unknown="value"/></rdf:RDF>"#;
        let parsed = parse_xmp(xmp).expect("RDF/XML accepts non-syntax RDF attributes");
        assert_eq!(
            parsed.undefined_extension_xmp_properties,
            BTreeSet::from([format!("{{{RDF_NAMESPACE}}}unknown")])
        );
    }

    #[test]
    fn rejects_duplicate_named_properties_across_descriptions() {
        let xmp = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:pdf="http://ns.adobe.com/pdf/1.3/">
          <rdf:Description pdf:Producer="first"/>
          <rdf:Description><pdf:Producer>second</pdf:Producer></rdf:Description>
        </rdf:RDF>"#;
        let error = parse_xmp(xmp).expect_err("duplicate top-level property must be rejected");
        assert!(error.contains("declared more than once"), "{error}");
    }

    #[test]
    fn predefined_property_checks_exclude_structured_fields() {
        let xmp = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:exif="http://ns.adobe.com/exif/1.0/">
          <rdf:Description><exif:Flash rdf:parseType="Resource">
          <exif:Fired>True</exif:Fired></exif:Flash></rdf:Description>
        </rdf:RDF>"#;
        let parsed = parse_xmp(xmp).expect("valid XMP");
        assert!(parsed.invalid_predefined_xmp_properties.is_empty());
        assert!(parsed.invalid_predefined_xmp_value_types.is_empty());
    }

    #[test]
    fn a_well_formed_xml_document_without_rdf_is_an_empty_xmp_package() {
        let parsed = parse_xmp(b"<not-xmp/>").expect("veraPDF returns an empty metadata model");
        assert!(!parsed.pdfa_identification_present);
        assert!(!parsed.packet_header_has_bytes);
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
    fn applies_pinned_latin1_and_ascii_control_recovery() {
        let mut latin1 = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:pdf="http://ns.adobe.com/pdf/1.3/"><rdf:Description pdf:Producer="caf"#
            .to_vec();
        latin1.push(0xE9);
        latin1.extend_from_slice(br#""/></rdf:RDF>"#);
        assert_eq!(parse_xmp(&latin1).expect("recovered Latin-1").producers, ["café"]);

        let controls = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:pdf="http://ns.adobe.com/pdf/1.3/"><rdf:Description pdf:Producer="a&#x1;b"/>
          </rdf:RDF>"#;
        assert_eq!(
            parse_xmp(controls).expect("recovered control").producers,
            ["a b"]
        );
        let del = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:pdf="http://ns.adobe.com/pdf/1.3/"><rdf:Description pdf:Producer="a&#x7F;b"/>
          </rdf:RDF>"#;
        assert_eq!(
            parse_xmp(del).expect("DEL is XML-valid").producers,
            ["a\u{7f}b"]
        );
    }

    #[test]
    fn detects_utf16_and_utf32_xmp_without_requiring_a_bom() {
        let xml = "<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\" \
          xmlns:pdf=\"http://ns.adobe.com/pdf/1.3/\"><rdf:Description pdf:Producer=\"ok\"/></rdf:RDF>";
        let utf16le = xml.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>();
        assert_eq!(
            parse_xmp(&utf16le).expect("UTF-16LE").producers,
            ["ok"]
        );
        let utf32be = xml
            .chars()
            .flat_map(|character| u32::from(character).to_be_bytes())
            .collect::<Vec<_>>();
        assert_eq!(
            parse_xmp(&utf32be).expect("UTF-32BE").producers,
            ["ok"]
        );
    }

    #[test]
    fn bounds_xml_depth_before_recursive_xmp_selection() {
        let mut xmp = "<x>".repeat(MAX_XMP_XML_DEPTH + 1);
        xmp.push_str("<leaf/>");
        xmp.push_str(&"</x>".repeat(MAX_XMP_XML_DEPTH + 1));
        let error = parse_xmp(xmp.as_bytes()).expect_err("deep XML must be bounded");
        assert!(error.contains("nesting depth"), "{error}");
    }

    #[test]
    fn bounds_xml_node_allocations() {
        let mut xmp = String::from("<x>");
        xmp.push_str(&"<n/>".repeat(MAX_XMP_XML_NODES as usize));
        xmp.push_str("</x>");
        assert!(parse_xmp(xmp.as_bytes()).is_err());
    }

    #[test]
    fn rejects_unqualified_rdf_description_attributes() {
        let xmp = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/">
          <rdf:Description bytes="123" pdfaid:part="1"/>
        </rdf:RDF>"#;
        let error = parse_xmp(xmp).expect_err("bare RDF/XML attribute must be rejected");
        assert!(error.contains("no XML namespace"), "{error}");
    }

    #[test]
    fn normalizes_the_pinned_extension_schema_value_type_names() {
        let known = BTreeSet::from([
            "gpscoordinate".to_owned(),
            "rational".to_owned(),
            "text".to_owned(),
        ]);
        for value_type in [
            "GPSCoordinate",
            "rational",
            "Choice Rational",
            "Bag Rational",
            "Open Choice of Bag Rational",
        ] {
            assert!(xmp_type_is_known(value_type, &known), "{value_type}");
        }
        assert_eq!(xmp_type_key("prefix Choice of Rational"), "prefix rational");
        assert_eq!(xmp_type_key("Closed Choice"), "text");
        assert!(!xmp_type_is_known("Undefined", &known));
        assert_eq!(xmp_type_key("\u{a0}Text\u{a0}"), "\u{a0}text\u{a0}");
    }

    #[test]
    fn validates_xpath_syntax_with_an_xpath_1_compiler() {
        assert!(scalar_xmp_value_matches(
            "/rdf:RDF/rdf:Description",
            "xpath"
        ));
        assert!(!scalar_xmp_value_matches("//*[", "xpath"));
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
