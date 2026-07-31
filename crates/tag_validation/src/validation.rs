use std::fmt;
use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::limits::SafetyLimits;
use crate::metadata::dates_equivalent;
use crate::model::{PdfDocument, PdfObjectId};
use crate::report::{
    FailureCategory, RuleFailure, ValidationCounts, ValidationFailure, ValidationReport,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ValidationProfile {
    #[serde(rename = "pdfa-1b")]
    PdfA1b,
}

impl fmt::Display for ValidationProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PdfA1b => formatter.write_str("PDF/A-1b"),
        }
    }
}

/// The number of validation rules implemented by [`ValidationProfile::PdfA1b`].
const TOTAL_RULE_COUNT: usize = 134;

/// Returns the sole element of a slice known to hold at most one entry, or
/// `None` for zero or multiple entries.
fn only<T>(items: &[T]) -> Option<&T> {
    (items.len() == 1).then(|| &items[0])
}

pub fn validate_file(
    path: &Path,
    profile: ValidationProfile,
    limits: &SafetyLimits,
) -> ValidationReport {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => {
            return ValidationReport::operational_failure(
                profile,
                "INPUT-IO-001",
                error.to_string(),
            );
        }
    };
    if metadata.len() > limits.max_input_size {
        return ValidationReport::operational_failure(
            profile,
            "RESOURCE-LIMIT-001",
            format!(
                "input is {} bytes, exceeding the {}-byte limit",
                metadata.len(),
                limits.max_input_size
            ),
        );
    }
    match fs::read(path) {
        Ok(bytes) => validate_bytes(&bytes, profile, limits),
        Err(error) => {
            ValidationReport::operational_failure(profile, "INPUT-IO-001", error.to_string())
        }
    }
}

pub fn validate_bytes(
    bytes: &[u8],
    profile: ValidationProfile,
    limits: &SafetyLimits,
) -> ValidationReport {
    let (document, inspections) = match PdfDocument::from_bytes_with_inspections(bytes, limits) {
        Ok(document) => document,
        Err(error) if error.is_safety_limit() => {
            return ValidationReport::operational_failure(
                profile,
                "RESOURCE-LIMIT-001",
                error.to_string(),
            );
        }
        Err(error) => return ValidationReport::parse_failure(profile, error.to_string()),
    };
    validate_document(document, inspections, profile)
}

fn validate_document(
    document: PdfDocument,
    inspections: crate::model::InspectionSummary,
    profile: ValidationProfile,
) -> ValidationReport {
    let mut failures = Vec::new();

    if document.encrypted {
        failures.push(failure(
            "PDFA1B-ENCRYPTION-001",
            "PDF/A-1b does not permit encryption",
            None,
            FailureCategory::Conformance,
        ));
        return finish_report(document, profile, failures, 2);
    }
    validate_header(&inspections.header, &mut failures);
    let has_trailer_id = if inspections.header.is_linearized {
        inspections.header.has_first_linearized_trailer_id
    } else {
        inspections.header.last_trailer_id.is_some() || document.trailer_id.is_some()
    };
    if !has_trailer_id {
        failures.push(failure(
            "PDFA1B-TRAILER-ID-001",
            "the applicable document trailer does not contain an ID entry",
            None,
            FailureCategory::Conformance,
        ));
    }
    if inspections.header.is_linearized
        && inspections.header.last_trailer_id.is_some()
        && inspections.header.first_linearized_trailer_id != inspections.header.last_trailer_id
    {
        failures.push(failure(
            "PDFA1B-LINEARIZED-TRAILER-ID-001",
            "the first-page and last trailer ID values differ in a linearized PDF",
            None,
            FailureCategory::Conformance,
        ));
    }
    if !document.catalog_present {
        failures.push(failure(
            "PDFA1B-CATALOG-001",
            "document trailer does not resolve to a Catalog dictionary",
            document.catalog_reference,
            FailureCategory::Conformance,
        ));
    }

    let metadata = &document.catalog_metadata;
    if !metadata.is_valid() {
        failures.push(failure(
            "PDFA1B-METADATA-STRUCTURE-001",
            "catalog Metadata must resolve to a stream with /Type /Metadata and /Subtype /XML",
            document.xmp_object,
            FailureCategory::Metadata,
        ));
    }
    if metadata.is_stream && metadata.has_filter {
        failures.push(failure(
            "PDFA1B-METADATA-FILTER-001",
            "the catalog metadata stream dictionary contains a Filter key",
            document.xmp_object,
            FailureCategory::Metadata,
        ));
    }

    if let Some(error) = &document.xmp_parse_error {
        failures.push(failure(
            "PDFA1B-XMP-001",
            format!("XMP metadata cannot be parsed: {error}"),
            document.xmp_object,
            FailureCategory::Metadata,
        ));
    }

    let xmp = document.xmp.as_ref();
    if xmp.is_some_and(|xmp| xmp.packet_header_has_bytes) {
        failures.push(failure(
            "PDFA1B-XMP-PACKET-BYTES-001",
            "the XMP packet header contains the forbidden bytes attribute",
            document.xmp_object,
            FailureCategory::Metadata,
        ));
    }
    if xmp.is_some_and(|xmp| xmp.packet_header_has_encoding) {
        failures.push(failure(
            "PDFA1B-XMP-PACKET-ENCODING-001",
            "the XMP packet header contains the forbidden encoding attribute",
            document.xmp_object,
            FailureCategory::Metadata,
        ));
    }
    if let Some(xmp) = xmp {
        if !xmp.invalid_predefined_xmp_properties.is_empty() {
            failures.push(failure(
                "PDFA1B-XMP-PREDEFINED-PROPERTY-001",
                format!(
                    "XMP uses undefined predefined-schema properties: {}",
                    xmp.invalid_predefined_xmp_properties
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                document.xmp_object,
                FailureCategory::Metadata,
            ));
        }
        if !xmp.invalid_predefined_xmp_value_types.is_empty() {
            failures.push(failure(
                "PDFA1B-XMP-PREDEFINED-VALUE-TYPE-001",
                format!(
                    "XMP predefined-schema properties use incompatible value shapes: {}",
                    xmp.invalid_predefined_xmp_value_types
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                document.xmp_object,
                FailureCategory::Metadata,
            ));
        }
        if !xmp.undefined_extension_xmp_properties.is_empty() {
            failures.push(failure(
                "PDFA1B-XMP-EXTENSION-PROPERTY-DEFINITION-001",
                format!(
                    "XMP extension properties are absent from the current extension schemas: {}",
                    xmp.undefined_extension_xmp_properties
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                document.xmp_object,
                FailureCategory::Metadata,
            ));
        }
        if !xmp.invalid_extension_xmp_value_types.is_empty() {
            failures.push(failure(
                "PDFA1B-XMP-EXTENSION-PROPERTY-VALUE-SHAPE-001",
                format!(
                    "XMP extension properties use incompatible declared value shapes: {}",
                    xmp.invalid_extension_xmp_value_types
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                document.xmp_object,
                FailureCategory::Metadata,
            ));
        }
        for test in &xmp.extension_schema_failed_tests {
            let (rule_id, message) = extension_schema_rule(*test);
            failures.push(failure(
                rule_id,
                message,
                document.xmp_object,
                FailureCategory::Metadata,
            ));
        }
        for test in &xmp.identification_prefix_failed_tests {
            let (rule_id, property) = identification_prefix_rule(*test);
            failures.push(failure(
                rule_id,
                format!(
                    "the PDF/A identification {property} property uses a lexical prefix other than pdfaid"
                ),
                document.xmp_object,
                FailureCategory::Metadata,
            ));
        }
    }
    if !xmp.is_some_and(|xmp| xmp.pdfa_identification_present) {
        failures.push(failure(
            "PDFA1B-ID-SCHEMA-001",
            "XMP does not contain the PDF/A Identification schema",
            document.xmp_object,
            FailureCategory::Metadata,
        ));
    }

    if xmp.is_some_and(|xmp| xmp.pdfa_identification_present) {
        if let Some(failure) = require_single_declared_value(
            xmp.map(|xmp| xmp.pdfa_parts.as_slice()),
            |value| value == "1",
            "PDFA1B-ID-PART-001",
            "PDF/A part",
            "pdfaid:part",
            "one value 1",
            document.xmp_object,
        ) {
            failures.push(failure);
        }

        if let Some(failure) = require_single_declared_value(
            xmp.map(|xmp| xmp.pdfa_conformances.as_slice()),
            |value| matches!(value, "A" | "B"),
            "PDFA1B-ID-CONFORMANCE-001",
            "PDF/A conformance",
            "pdfaid:conformance",
            "one A or B",
            document.xmp_object,
        ) {
            failures.push(failure);
        }
    }

    validate_info_consistency(&document, &mut failures);

    validate_output_intents(&document, &mut failures);

    aggregate_failures_with_location(
        &inspections.icc_based.failures,
        "PDFA1B-ICCBASED-001",
        Some("direct profile"),
        &mut failures,
    );
    aggregate_failures_with_location(
        &inspections.icc_based.component_failures,
        "PDFA1B-ICCBASED-COMPONENTS-001",
        Some("direct profile"),
        &mut failures,
    );
    aggregate_failures_with_location(
        &inspections.icc_based.invalid_devicen_components,
        "PDFA1B-DEVICEN-COMPONENTS-001",
        Some("direct DeviceN space"),
        &mut failures,
    );
    let output_color_space = pdfa_output_color_space(&document);
    validate_device_color_spaces(output_color_space, &inspections.icc_based, &mut failures);
    validate_xobjects(&inspections.xobjects, &mut failures);
    validate_graphics(&inspections.graphics, &inspections.content, &mut failures);
    validate_annotations(output_color_space, &inspections.annotations, &mut failures);
    validate_actions(&inspections.actions, &mut failures);
    validate_forms(&inspections.forms, &mut failures);
    validate_document_features(&inspections.document_features, &mut failures);
    validate_file_specifications(
        &inspections.document_features,
        &inspections.actions,
        &mut failures,
    );
    validate_object_limits(&inspections.object_limits, &mut failures);
    validate_stream_safety(&inspections.stream_safety, &mut failures);

    validate_font_dictionaries(&inspections.font_embedding, &mut failures);
    validate_font_embedding(&inspections.font_embedding, &mut failures);

    finish_report(document, profile, failures, TOTAL_RULE_COUNT)
}

fn validate_header(header: &crate::syntax::HeaderSummary, failures: &mut Vec<ValidationFailure>) {
    if !header.has_valid_header {
        failures.push(failure(
            "PDFA1B-HEADER-001",
            "the file must start with a PDF header in the form %PDF-n.m",
            None,
            FailureCategory::Conformance,
        ));
    }
    if !header.has_binary_comment {
        failures.push(failure(
            "PDFA1B-HEADER-BINARY-COMMENT-001",
            "the PDF header must be immediately followed by a comment with four bytes above 127",
            None,
            FailureCategory::Conformance,
        ));
    }
    if header.has_post_eof_data {
        failures.push(failure(
            "PDFA1B-POST-EOF-DATA-001",
            "data follows the last %%EOF marker",
            None,
            FailureCategory::Conformance,
        ));
    }
}

fn validate_object_limits(
    limits: &crate::object_limits::ObjectLimitsSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    let checks = [
        (
            "PDFA1B-INTEGER-RANGE-001",
            "an integer is outside the inclusive PDF/A-1 range",
            &limits.out_of_range_integers,
        ),
        (
            "PDFA1B-REAL-RANGE-001",
            "a real number is outside the inclusive PDF/A-1 range",
            &limits.out_of_range_reals,
        ),
        (
            "PDFA1B-STRING-LENGTH-001",
            "a string exceeds the 65,535-byte PDF/A-1 limit",
            &limits.overlong_strings,
        ),
        (
            "PDFA1B-NAME-LENGTH-001",
            "a name exceeds the 127-byte PDF/A-1 limit",
            &limits.overlong_names,
        ),
        (
            "PDFA1B-ARRAY-LENGTH-001",
            "an array exceeds the 8,191-entry PDF/A-1 limit",
            &limits.oversized_arrays,
        ),
        (
            "PDFA1B-DICTIONARY-LENGTH-001",
            "a dictionary exceeds the 4,095-entry PDF/A-1 limit",
            &limits.oversized_dictionaries,
        ),
    ];
    for (rule_id, description, objects) in checks {
        if !objects.is_empty() {
            failures.push(failure(
                rule_id,
                description,
                only(objects).copied(),
                FailureCategory::Conformance,
            ));
        }
    }
    if limits.too_many_indirect_objects {
        failures.push(failure(
            "PDFA1B-INDIRECT-OBJECT-COUNT-001",
            "the document exceeds the 8,388,607 indirect-object PDF/A-1 limit",
            None,
            FailureCategory::Conformance,
        ));
    }
}

fn extension_schema_rule(test: u8) -> (&'static str, &'static str) {
    match test {
        1 => (
            "PDFA1B-XMP-EXTENSION-FIELDS-001",
            "an XMP extension-schema object contains a field not defined by PDF/A-1",
        ),
        2 => (
            "PDFA1B-XMP-EXTENSION-CONTAINER-001",
            "pdfaExtension:schemas must use the pdfaExtension prefix and an rdf:Bag",
        ),
        3 => (
            "PDFA1B-XMP-EXTENSION-SCHEMA-NAME-001",
            "an extension schema definition has an invalid pdfaSchema:schema field",
        ),
        4 => (
            "PDFA1B-XMP-EXTENSION-SCHEMA-NAMESPACE-001",
            "an extension schema definition has an invalid pdfaSchema:namespaceURI field",
        ),
        5 => (
            "PDFA1B-XMP-EXTENSION-SCHEMA-PREFIX-001",
            "an extension schema definition has an invalid pdfaSchema:prefix field",
        ),
        6 => (
            "PDFA1B-XMP-EXTENSION-SCHEMA-PROPERTIES-001",
            "an extension schema definition has an invalid property sequence",
        ),
        7 => (
            "PDFA1B-XMP-EXTENSION-SCHEMA-VALUE-TYPES-001",
            "an extension schema definition has an invalid value-type sequence",
        ),
        8 => (
            "PDFA1B-XMP-EXTENSION-PROPERTY-NAME-001",
            "an extension-schema property has an invalid pdfaProperty:name field",
        ),
        9 => (
            "PDFA1B-XMP-EXTENSION-PROPERTY-VALUE-TYPE-001",
            "an extension-schema property has an invalid or undefined value type",
        ),
        10 => (
            "PDFA1B-XMP-EXTENSION-PROPERTY-CATEGORY-001",
            "an extension-schema property has an invalid category",
        ),
        11 => (
            "PDFA1B-XMP-EXTENSION-PROPERTY-DESCRIPTION-001",
            "an extension-schema property has an invalid description",
        ),
        12 => (
            "PDFA1B-XMP-EXTENSION-VALUE-TYPE-NAME-001",
            "an extension-schema value type has an invalid pdfaType:type field",
        ),
        13 => (
            "PDFA1B-XMP-EXTENSION-VALUE-TYPE-NAMESPACE-001",
            "an extension-schema value type has an invalid namespace URI",
        ),
        14 => (
            "PDFA1B-XMP-EXTENSION-VALUE-TYPE-PREFIX-001",
            "an extension-schema value type has an invalid prefix field",
        ),
        15 => (
            "PDFA1B-XMP-EXTENSION-VALUE-TYPE-DESCRIPTION-001",
            "an extension-schema value type has an invalid description",
        ),
        16 => (
            "PDFA1B-XMP-EXTENSION-VALUE-TYPE-FIELDS-001",
            "an extension-schema value type has an invalid field sequence",
        ),
        17 => (
            "PDFA1B-XMP-EXTENSION-FIELD-NAME-001",
            "an extension-schema field has an invalid name",
        ),
        18 => (
            "PDFA1B-XMP-EXTENSION-FIELD-VALUE-TYPE-001",
            "an extension-schema field has an invalid or undefined value type",
        ),
        19 => (
            "PDFA1B-XMP-EXTENSION-FIELD-DESCRIPTION-001",
            "an extension-schema field has an invalid description",
        ),
        _ => unreachable!("unsupported PDF/A-1 extension-schema test {test}"),
    }
}

fn identification_prefix_rule(test: u8) -> (&'static str, &'static str) {
    match test {
        4 => ("PDFA1B-ID-PART-PREFIX-001", "part"),
        5 => ("PDFA1B-ID-CONFORMANCE-PREFIX-001", "conformance"),
        6 => ("PDFA1B-ID-AMD-PREFIX-001", "amd"),
        _ => unreachable!("unsupported PDF/A-1 identification-prefix test {test}"),
    }
}

fn validate_actions(
    actions: &crate::actions::ActionSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    for (invalid, rule_id) in [
        (
            actions.invalid_action_types.as_slice(),
            "PDFA1B-ACTION-TYPE-001",
        ),
        (
            actions.invalid_named_actions.as_slice(),
            "PDFA1B-NAMED-ACTION-001",
        ),
        (
            actions.widgets_with_actions.as_slice(),
            "PDFA1B-WIDGET-ACTION-001",
        ),
        (
            actions.widgets_with_additional_actions.as_slice(),
            "PDFA1B-WIDGET-ADDITIONAL-ACTIONS-001",
        ),
        (
            actions.fields_with_additional_actions.as_slice(),
            "PDFA1B-FIELD-ADDITIONAL-ACTIONS-001",
        ),
        (
            actions.catalog_with_additional_actions.as_slice(),
            "PDFA1B-CATALOG-ADDITIONAL-ACTIONS-001",
        ),
    ] {
        aggregate_failures(invalid, rule_id, failures);
    }
}

fn validate_forms(forms: &crate::forms::FormSummary, failures: &mut Vec<ValidationFailure>) {
    for (invalid, rule_id) in [
        (
            forms.invalid_need_appearances.as_slice(),
            "PDFA1B-ACROFORM-NEED-APPEARANCES-001",
        ),
        (
            forms.widgets_without_appearances.as_slice(),
            "PDFA1B-WIDGET-APPEARANCE-001",
        ),
    ] {
        aggregate_failures(invalid, rule_id, failures);
    }
}

fn validate_document_features(
    features: &crate::document_features::DocumentFeatureSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    for (invalid, rule_id, description) in [
        (
            features.contains_embedded_files_name,
            "PDFA1B-NAMES-EMBEDDED-FILES-001",
            "the catalog Names dictionary contains an EmbeddedFiles entry",
        ),
        (
            features.contains_optional_content,
            "PDFA1B-OPTIONAL-CONTENT-001",
            "the document catalog contains an OCProperties entry",
        ),
    ] {
        if invalid {
            failures.push(failure(
                rule_id,
                description,
                features.catalog_id,
                FailureCategory::Conformance,
            ));
        }
    }
}

/// Aggregates `PDFA1B-FILE-SPEC-EMBEDDED-FILE-001` failures across every
/// reachability path veraPDF's `CosFileSpecification` object covers: the
/// catalog `Names/EmbeddedFiles` name tree, and `GoToR`/`SubmitForm` action
/// `/F` entries.
fn validate_file_specifications(
    document_features: &crate::document_features::DocumentFeatureSummary,
    actions: &crate::actions::ActionSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    let file_spec_failures = document_features
        .file_specs_with_embedded_files
        .iter()
        .chain(&actions.file_specs_with_embedded_files)
        .cloned()
        .collect::<Vec<_>>();
    aggregate_failures(
        &file_spec_failures,
        "PDFA1B-FILE-SPEC-EMBEDDED-FILE-001",
        failures,
    );
}

fn validate_stream_safety(
    streams: &crate::stream_safety::StreamSafetySummary,
    failures: &mut Vec<ValidationFailure>,
) {
    if !streams.external_stream_entries.is_empty() {
        let object_id = only(&streams.external_stream_entries).map(|entry| entry.object_id);
        let description = streams
            .external_stream_entries
            .iter()
            .map(|stream| {
                format!(
                    "stream object {} {} contains {}",
                    stream.object_id.object_number,
                    stream.object_id.generation,
                    stream.keys.join(", ")
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        failures.push(failure(
            "PDFA1B-STREAM-EXTERNAL-DATA-001",
            description,
            object_id,
            FailureCategory::Conformance,
        ));
    }
    if !streams.lzw_filters.is_empty() {
        let object_id = only(&streams.lzw_filters).copied();
        failures.push(failure(
            "PDFA1B-STREAM-LZW-001",
            "a parsed stream declares the forbidden LZWDecode filter",
            object_id,
            FailureCategory::Conformance,
        ));
    }
    if !streams.xref_streams.is_empty() {
        let object_id = only(&streams.xref_streams).copied();
        failures.push(failure(
            "PDFA1B-XREF-STREAM-001",
            "the document contains an xref stream",
            object_id,
            FailureCategory::Conformance,
        ));
    }
    for (invalid, rule_id, message) in [
        (
            streams.has_odd_hex_string,
            "PDFA1B-HEX-STRING-LENGTH-001",
            "a hexadecimal string contains an odd number of non-whitespace characters",
        ),
        (
            streams.has_non_hex_character,
            "PDFA1B-HEX-STRING-CHARACTERS-001",
            "a hexadecimal string contains a non-hexadecimal character",
        ),
        (
            streams.has_invalid_xref_subsection_spacing,
            "PDFA1B-XREF-SUBSECTION-SPACING-001",
            "an xref subsection header does not separate its numbers with one SPACE character",
        ),
        (
            streams.has_invalid_xref_eol,
            "PDFA1B-XREF-EOL-001",
            "the xref keyword is not followed by exactly one EOL marker",
        ),
        (
            streams.has_invalid_indirect_object_syntax,
            "PDFA1B-INDIRECT-OBJECT-SYNTAX-001",
            "an indirect object header or endobj keyword has invalid PDF/A-1 spacing",
        ),
    ] {
        if invalid {
            failures.push(failure(
                rule_id,
                message,
                None,
                FailureCategory::Conformance,
            ));
        }
    }
    for (invalid, rule_id, message) in [
        (
            streams.invalid_lengths.as_slice(),
            "PDFA1B-STREAM-LENGTH-001",
            "a stream /Length does not match its raw content length",
        ),
        (
            streams.invalid_eol_markers.as_slice(),
            "PDFA1B-STREAM-EOL-001",
            "a stream has invalid EOL markers around stream data",
        ),
    ] {
        if !invalid.is_empty() {
            failures.push(failure(
                rule_id,
                message,
                only(invalid).copied(),
                FailureCategory::Conformance,
            ));
        }
    }
}

/// Computes the single object id (when exactly one entry is present) and the
/// `"; "`-joined description used by every same-rule failure aggregator.
fn joined_failure(invalid: &[RuleFailure]) -> (Option<PdfObjectId>, String) {
    let object_id = only(invalid).and_then(|entry| entry.object_id);
    let description = invalid
        .iter()
        .map(|entry| entry.description.as_str())
        .collect::<Vec<_>>()
        .join("; ");
    (object_id, description)
}

/// Aggregates same-rule failures into one [`ValidationFailure`], attaching
/// the single object id only when exactly one entry is present.
fn aggregate_failures(
    invalid: &[RuleFailure],
    rule_id: &'static str,
    failures: &mut Vec<ValidationFailure>,
) {
    if invalid.is_empty() {
        return;
    }
    let (object_id, description) = joined_failure(invalid);
    failures.push(failure(
        rule_id,
        description,
        object_id,
        FailureCategory::Conformance,
    ));
}

/// Like [`aggregate_failures`], but prefixes each entry with its indirect
/// object identity when known, or with `no_id_label` (if given) otherwise.
fn aggregate_failures_with_location(
    invalid: &[RuleFailure],
    rule_id: &'static str,
    no_id_label: Option<&str>,
    failures: &mut Vec<ValidationFailure>,
) {
    if invalid.is_empty() {
        return;
    }
    let object_id = only(invalid).and_then(|entry| entry.object_id);
    let detail = invalid
        .iter()
        .map(|entry| match entry.object_id {
            Some(object_id) => format!(
                "{} {}: {}",
                object_id.object_number, object_id.generation, entry.description
            ),
            None => match no_id_label {
                Some(label) => format!("{label}: {}", entry.description),
                None => entry.description.clone(),
            },
        })
        .collect::<Vec<_>>()
        .join("; ");
    failures.push(failure(
        rule_id,
        detail,
        object_id,
        FailureCategory::Conformance,
    ));
}

fn validate_device_color_spaces(
    output_color_space: Option<&str>,
    color_spaces: &crate::icc_based::IccBasedSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    if let Some(context) = &color_spaces.device_rgb_context
        && output_color_space != Some("RGB ")
    {
        failures.push(failure(
            "PDFA1B-DEVICE-RGB-001",
            format!(
                "DeviceRGB is selected in {context}, but the PDF/A-1 output colour space is {output_color_space:?}"
            ),
            None,
            FailureCategory::Conformance,
        ));
    }
    if let Some(context) = &color_spaces.device_cmyk_context
        && output_color_space != Some("CMYK")
    {
        failures.push(failure(
            "PDFA1B-DEVICE-CMYK-001",
            format!(
                "DeviceCMYK is selected in {context}, but the PDF/A-1 output colour space is {output_color_space:?}"
            ),
            None,
            FailureCategory::Conformance,
        ));
    }
    if let Some(context) = &color_spaces.device_gray_context
        && output_color_space.is_none()
    {
        failures.push(failure(
            "PDFA1B-DEVICE-GRAY-001",
            format!(
                "DeviceGray is selected in {context}, but no PDF/A-1 output colour space is defined"
            ),
            None,
            FailureCategory::Conformance,
        ));
    }
}

fn pdfa_output_color_space(document: &PdfDocument) -> Option<&str> {
    let mut output_color_space = None;
    for entry in document
        .output_intents_summary
        .entries
        .iter()
        .filter(|entry| entry.is_dictionary_based && entry.dest_output_profile_is_stream)
    {
        if entry.subtype.as_deref() == Some("GTS_PDFA1") {
            output_color_space = entry
                .dest_output_profile_header
                .as_ref()
                .map(|header| header.color_space.as_str());
        }
    }
    output_color_space
}

fn validate_xobjects(
    xobjects: &crate::xobject::XObjectSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    for (invalid, rule_id) in [
        (
            xobjects.image_alternates.as_slice(),
            "PDFA1B-IMAGE-ALTERNATES-001",
        ),
        (xobjects.xobject_opi.as_slice(), "PDFA1B-XOBJECT-OPI-001"),
        (
            xobjects.image_interpolate.as_slice(),
            "PDFA1B-IMAGE-INTERPOLATE-001",
        ),
        (
            xobjects.image_bits_per_component.as_slice(),
            "PDFA1B-IMAGE-BPC-001",
        ),
        (
            xobjects.mask_bits_per_component.as_slice(),
            "PDFA1B-IMAGE-MASK-BPC-001",
        ),
        (
            xobjects.form_postscript.as_slice(),
            "PDFA1B-FORM-POSTSCRIPT-001",
        ),
        (
            xobjects.form_reference.as_slice(),
            "PDFA1B-FORM-REFERENCE-001",
        ),
        (
            xobjects.postscript_xobject.as_slice(),
            "PDFA1B-XOBJECT-POSTSCRIPT-001",
        ),
    ] {
        aggregate_failures_with_location(invalid, rule_id, None, failures);
    }
}

fn validate_graphics(
    graphics: &crate::graphics::GraphicsSummary,
    content: &crate::content_support::ContentExecutionSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    for (invalid, rule_id) in [
        (
            graphics.transfer_functions.as_slice(),
            "PDFA1B-EXTGSTATE-TR-001",
        ),
        (
            graphics.transfer_functions_2.as_slice(),
            "PDFA1B-EXTGSTATE-TR2-001",
        ),
        (
            graphics.rendering_intents.as_slice(),
            "PDFA1B-RENDERING-INTENT-001",
        ),
        (
            graphics.extgstate_soft_masks.as_slice(),
            "PDFA1B-EXTGSTATE-SMASK-001",
        ),
        (
            graphics.xobject_soft_masks.as_slice(),
            "PDFA1B-XOBJECT-SMASK-001",
        ),
        (
            graphics.transparency_groups.as_slice(),
            "PDFA1B-TRANSPARENCY-GROUP-001",
        ),
        (
            graphics.blend_modes.as_slice(),
            "PDFA1B-EXTGSTATE-BLEND-MODE-001",
        ),
        (
            graphics.stroke_alpha.as_slice(),
            "PDFA1B-EXTGSTATE-STROKE-ALPHA-001",
        ),
        (
            graphics.fill_alpha.as_slice(),
            "PDFA1B-EXTGSTATE-FILL-ALPHA-001",
        ),
    ] {
        aggregate_failures_with_location(invalid, rule_id, None, failures);
    }
    if !content.undefined_operators.is_empty() {
        let detail = content
            .undefined_operators
            .iter()
            .map(|(operator, context)| format!("{context}: {operator}"))
            .collect::<Vec<_>>()
            .join("; ");
        failures.push(failure(
            "PDFA1B-CONTENT-OPERATOR-001",
            format!("content uses operators not defined by PDF 1.4: {detail}"),
            None,
            FailureCategory::Conformance,
        ));
    }
    if let Some(context) = &content.inline_image_lzw_context {
        failures.push(failure(
            "PDFA1B-INLINE-IMAGE-LZW-001",
            format!("{context} declares the forbidden LZW inline-image filter"),
            None,
            FailureCategory::Conformance,
        ));
    }
}

fn validate_annotations(
    output_color_space: Option<&str>,
    annotations: &crate::annotations::AnnotationSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    for (invalid, rule_id) in [
        (
            annotations.invalid_subtypes.as_slice(),
            "PDFA1B-ANNOTATION-SUBTYPE-001",
        ),
        (
            annotations.invalid_opacities.as_slice(),
            "PDFA1B-ANNOTATION-OPACITY-001",
        ),
        (
            annotations.invalid_flags.as_slice(),
            "PDFA1B-ANNOTATION-FLAGS-001",
        ),
        (
            annotations.invalid_appearance_entries.as_slice(),
            "PDFA1B-ANNOTATION-AP-ENTRIES-001",
        ),
        (
            annotations.invalid_button_appearances.as_slice(),
            "PDFA1B-WIDGET-BUTTON-APPEARANCE-001",
        ),
        (
            annotations.invalid_other_appearances.as_slice(),
            "PDFA1B-ANNOTATION-NORMAL-APPEARANCE-001",
        ),
    ] {
        aggregate_failures(invalid, rule_id, failures);
    }
    if output_color_space != Some("RGB ") {
        aggregate_failures(
            &annotations.color_uses,
            "PDFA1B-ANNOTATION-COLOR-001",
            failures,
        );
    }
}

fn validate_font_embedding(
    font_embedding: &crate::font_embedding::FontEmbeddingSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    let invalid = &font_embedding.failures;
    if invalid.is_empty() {
        return;
    }
    let (object_id, message) = joined_failure(invalid);
    failures.push(failure(
        "PDFA1B-FONT-EMBEDDING-001",
        format!("font program is not embedded for {message}"),
        object_id,
        FailureCategory::Conformance,
    ));
}

fn validate_font_dictionaries(
    fonts: &crate::font_embedding::FontEmbeddingSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    for (invalid, rule_id) in [
        (fonts.invalid_types.as_slice(), "PDFA1B-FONT-TYPE-001"),
        (fonts.invalid_subtypes.as_slice(), "PDFA1B-FONT-SUBTYPE-001"),
        (
            fonts.invalid_base_fonts.as_slice(),
            "PDFA1B-FONT-BASEFONT-001",
        ),
        (
            fonts.invalid_first_chars.as_slice(),
            "PDFA1B-FONT-FIRSTCHAR-001",
        ),
        (
            fonts.invalid_last_chars.as_slice(),
            "PDFA1B-FONT-LASTCHAR-001",
        ),
        (fonts.invalid_widths.as_slice(), "PDFA1B-FONT-WIDTHS-001"),
        (
            fonts.invalid_font_file_subtypes.as_slice(),
            "PDFA1B-FONT-FILE-SUBTYPE-001",
        ),
        (
            fonts.incompatible_type0_system_info.as_slice(),
            "PDFA1B-TYPE0-CID-SYSTEM-INFO-001",
        ),
        (
            fonts.invalid_cid_to_gid_maps.as_slice(),
            "PDFA1B-CIDTOGIDMAP-001",
        ),
        (
            fonts.unembedded_cmaps.as_slice(),
            "PDFA1B-CMAP-EMBEDDING-001",
        ),
        (
            fonts.invalid_cmap_wmodes.as_slice(),
            "PDFA1B-CMAP-WMODE-001",
        ),
        (
            fonts.invalid_cmap_cids.as_slice(),
            "PDFA1B-CMAP-CID-RANGE-001",
        ),
        (
            fonts.oversized_cmap_cids.as_slice(),
            "PDFA1B-CMAP-MAX-CID-001",
        ),
        (
            fonts.invalid_type1_subset_charsets.as_slice(),
            "PDFA1B-TYPE1-SUBSET-CHARSET-001",
        ),
        (
            fonts.invalid_cid_subset_cidsets.as_slice(),
            "PDFA1B-CID-SUBSET-CIDSET-001",
        ),
        (
            fonts.invalid_nonsymbolic_truetype_encodings.as_slice(),
            "PDFA1B-TRUETYPE-NONSYMBOLIC-ENCODING-001",
        ),
        (
            fonts.invalid_symbolic_truetype_encodings.as_slice(),
            "PDFA1B-TRUETYPE-SYMBOLIC-ENCODING-001",
        ),
        (
            fonts.invalid_symbolic_truetype_cmaps.as_slice(),
            "PDFA1B-TRUETYPE-SYMBOLIC-CMAP-001",
        ),
        (
            fonts.missing_truetype_glyphs.as_slice(),
            "PDFA1B-TRUETYPE-GLYPH-PRESENCE-001",
        ),
        (
            fonts.missing_type1_glyphs.as_slice(),
            "PDFA1B-TYPE1-GLYPH-PRESENCE-001",
        ),
        (
            fonts.inconsistent_truetype_widths.as_slice(),
            "PDFA1B-TRUETYPE-GLYPH-WIDTH-001",
        ),
        (
            fonts.excessive_graphics_state_nesting.as_slice(),
            "PDFA1B-GRAPHICS-STATE-NESTING-001",
        ),
    ] {
        aggregate_failures(invalid, rule_id, failures);
    }
}

/// Requires exactly one declared value satisfying `accept`, producing a
/// failure describing either the rejected set of values or the absence of
/// any declaration.
fn require_single_declared_value(
    values: Option<&[String]>,
    accept: impl Fn(&str) -> bool,
    rule_id: &'static str,
    noun: &str,
    declaration_name: &str,
    expected_description: &str,
    object_id: Option<PdfObjectId>,
) -> Option<ValidationFailure> {
    match values {
        Some([value]) if accept(value) => None,
        Some(values) => Some(failure(
            rule_id,
            format!(
                "XMP declares {noun} values {values:?}, expected exactly {expected_description}"
            ),
            object_id,
            FailureCategory::Metadata,
        )),
        None => Some(failure(
            rule_id,
            format!("XMP has no {declaration_name} declaration"),
            object_id,
            FailureCategory::Metadata,
        )),
    }
}

fn validate_output_intents(document: &PdfDocument, failures: &mut Vec<ValidationFailure>) {
    validate_output_intent_profiles(document, failures);
    validate_output_intent_identity(document, failures);
}

fn validate_output_intent_profiles(document: &PdfDocument, failures: &mut Vec<ValidationFailure>) {
    let entries = document
        .output_intents_summary
        .entries
        .iter()
        .filter(|entry| entry.is_dictionary_based);

    let mut invalid_profiles = Vec::new();
    for entry in entries
        .clone()
        .filter(|entry| entry.dest_output_profile_is_stream)
    {
        if !entry
            .dest_output_profile_header
            .as_ref()
            .is_some_and(|header| header.conforms_to_pdfa_1_output_intent())
        {
            let detail = match (
                &entry.dest_output_profile_header,
                &entry.dest_output_profile_decode_error,
            ) {
                (Some(header), _) => format!(
                    "ICC output profile has class {:?}, colour space {:?}, and version {}.{}",
                    header.device_class,
                    header.color_space,
                    header.version_major,
                    header.version_minor
                ),
                (None, Some(error)) => error.clone(),
                (None, None) => {
                    "ICC output profile is shorter than the 20-byte header prefix required by this check"
                        .to_owned()
                }
            };
            invalid_profiles.push((entry.dest_output_profile_id, detail));
        }
    }
    if !invalid_profiles.is_empty() {
        let object_id = only(&invalid_profiles).and_then(|(object_id, _)| *object_id);
        let detail = invalid_profiles
            .iter()
            .map(|(object_id, detail)| match object_id {
                Some(object_id) => format!(
                    "{} {}: {detail}",
                    object_id.object_number, object_id.generation
                ),
                None => format!("direct profile: {detail}"),
            })
            .collect::<Vec<_>>()
            .join("; ");
        failures.push(failure(
            "PDFA1B-OUTPUTINTENT-001",
            detail,
            object_id,
            FailureCategory::Conformance,
        ));
    }
}

fn validate_output_intent_identity(document: &PdfDocument, failures: &mut Vec<ValidationFailure>) {
    let entries = document
        .output_intents_summary
        .entries
        .iter()
        .filter(|entry| entry.is_dictionary_based);

    let mut indirect_profiles = entries.filter_map(|entry| entry.dest_output_profile_id);
    if let Some(expected) = indirect_profiles.next()
        && let Some(actual) = indirect_profiles.find(|object_id| *object_id != expected)
    {
        failures.push(failure(
            "PDFA1B-OUTPUTINTENT-IDENTITY-001",
            format!(
                "output intents reference different indirect destination profiles: {} {} and {} {}",
                expected.object_number,
                expected.generation,
                actual.object_number,
                actual.generation
            ),
            document.catalog_reference,
            FailureCategory::Conformance,
        ));
    }
}

fn validate_info_consistency(document: &PdfDocument, failures: &mut Vec<ValidationFailure>) {
    let xmp = document.xmp.as_ref();
    let empty = &[];
    let object_id = document.info_object;
    compare_field(
        document,
        "Title",
        xmp.map_or(empty, |xmp| &xmp.title_x_default),
        "PDFA1B-INFO-TITLE-001",
        "dc:title['x-default']",
        object_id,
        failures,
        |a, b| a == b,
    );
    if let Some(author) = document.info.values.get("Author")
        && (xmp.is_none_or(|xmp| xmp.creator_container_count != 1)
            || xmp.is_none_or(|xmp| xmp.creators.len() != 1)
            || xmp.and_then(|xmp| xmp.creators.first()) != Some(author))
    {
        failures.push(failure(
            "PDFA1B-INFO-AUTHOR-001",
            format!(
                "Info Author {author:?} does not equal the sole dc:creator entry {:?}",
                xmp.map(|xmp| &xmp.creators)
            ),
            object_id,
            FailureCategory::Metadata,
        ));
    }
    compare_field(
        document,
        "Subject",
        xmp.map_or(empty, |xmp| &xmp.description_x_default),
        "PDFA1B-INFO-SUBJECT-001",
        "dc:description['x-default']",
        object_id,
        failures,
        |a, b| a == b,
    );
    compare_field(
        document,
        "Keywords",
        xmp.map_or(empty, |xmp| &xmp.keywords),
        "PDFA1B-INFO-KEYWORDS-001",
        "pdf:Keywords",
        object_id,
        failures,
        |a, b| a == b,
    );
    compare_field(
        document,
        "Creator",
        xmp.map_or(empty, |xmp| &xmp.creator_tools),
        "PDFA1B-INFO-CREATOR-001",
        "xmp:CreatorTool",
        object_id,
        failures,
        |a, b| a == b,
    );
    compare_field(
        document,
        "Producer",
        xmp.map_or(empty, |xmp| &xmp.producers),
        "PDFA1B-INFO-PRODUCER-001",
        "pdf:Producer",
        object_id,
        failures,
        |a, b| a == b,
    );
    if let Some(xmp) = xmp {
        if !xmp
            .invalid_predefined_xmp_value_types
            .contains("{http://ns.adobe.com/xap/1.0/}CreateDate (date)")
        {
            compare_field(
                document,
                "CreationDate",
                &xmp.create_dates,
                "PDFA1B-INFO-CREATIONDATE-001",
                "xmp:CreateDate",
                object_id,
                failures,
                dates_equivalent,
            );
        }
        if !xmp
            .invalid_predefined_xmp_value_types
            .contains("{http://ns.adobe.com/xap/1.0/}ModifyDate (date)")
        {
            compare_field(
                document,
                "ModDate",
                &xmp.modify_dates,
                "PDFA1B-INFO-MODDATE-001",
                "xmp:ModifyDate",
                object_id,
                failures,
                dates_equivalent,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn compare_field(
    document: &PdfDocument,
    info_key: &str,
    xmp_values: &[String],
    rule_id: &'static str,
    xmp_name: &str,
    object_id: Option<PdfObjectId>,
    failures: &mut Vec<ValidationFailure>,
    matches: impl Fn(&str, &str) -> bool,
) {
    if let Some(info_value) = document.info.values.get(info_key)
        && (xmp_values.len() != 1
            || !xmp_values
                .first()
                .is_some_and(|xmp_value| matches(info_value, xmp_value)))
    {
        failures.push(failure(
            rule_id,
            format!(
                "Info {info_key} {info_value:?} is not equivalent to {xmp_name} {xmp_values:?}"
            ),
            object_id,
            FailureCategory::Metadata,
        ));
    }
}

fn finish_report(
    document: PdfDocument,
    profile: ValidationProfile,
    mut failures: Vec<ValidationFailure>,
    total_checks: usize,
) -> ValidationReport {
    failures.sort_by_key(|failure| failure.rule_id);
    let failed = failures.len();
    ValidationReport {
        profile,
        implemented_checks_passed: failures.is_empty(),
        preliminary: true,
        checks: ValidationCounts {
            total: total_checks,
            passed: total_checks.saturating_sub(failed),
            failed,
        },
        document: Some(document),
        failures,
    }
}

fn failure(
    rule_id: &'static str,
    message: impl Into<String>,
    object_id: Option<PdfObjectId>,
    category: FailureCategory,
) -> ValidationFailure {
    ValidationFailure {
        rule_id,
        message: message.into(),
        object_id,
        category,
    }
}

#[cfg(test)]
mod tests {
    use lopdf::xref::XrefType;
    use lopdf::{Dictionary, Document, Object, Stream, StringFormat, dictionary};

    use super::*;

    const VALID_XMP: &[u8] = br#"<?xpacket begin=""?>
      <x:xmpmeta xmlns:x="adobe:ns:meta/">
        <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
          <rdf:Description xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/"
            pdfaid:part="1" pdfaid:conformance="B"/>
        </rdf:RDF>
      </x:xmpmeta>
      <?xpacket end="w"?>"#;

    const COMPLETE_XMP: &[u8] = br#"<?xpacket begin=""?>
      <x:xmpmeta xmlns:x="adobe:ns:meta/">
        <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
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
        </rdf:RDF>
      </x:xmpmeta>
      <?xpacket end="w"?>"#;

    fn pdf_document() -> Document {
        let mut document = Document::with_version("1.4");
        document.reference_table.cross_reference_type = XrefType::CrossReferenceTable;
        document.trailer.set(
            "ID",
            vec![
                Object::string_literal("0123456789abcdef"),
                Object::string_literal("0123456789abcdef"),
            ],
        );
        document
    }

    #[test]
    fn accepts_all_implemented_checks() {
        let bytes = fixture(Some(VALID_XMP), true);
        let report = validate_bytes(&bytes, ValidationProfile::PdfA1b, &SafetyLimits::default());
        assert!(report.implemented_checks_passed, "{:#?}", report.failures);
        assert_eq!(report.checks.passed, TOTAL_RULE_COUNT);
    }

    #[test]
    fn rejects_a_missing_binary_header_comment() {
        let mut bytes = fixture(Some(VALID_XMP), true);
        let line_end = bytes
            .iter()
            .position(|byte| matches!(byte, b'\r' | b'\n'))
            .expect("PDF header line ending");
        let comment_start = if bytes[line_end] == b'\r' && bytes.get(line_end + 1) == Some(&b'\n') {
            line_end + 2
        } else {
            line_end + 1
        };
        bytes[comment_start + 1..comment_start + 5].copy_from_slice(b"abcd");

        let report = validate_bytes(&bytes, ValidationProfile::PdfA1b, &SafetyLimits::default());
        assert_rule(&report, "PDFA1B-HEADER-BINARY-COMMENT-001");
        assert_no_rule(&report, "PDFA1B-HEADER-001");
    }

    #[test]
    fn rejects_mismatched_linearized_trailer_ids() {
        let bytes = fixture(Some(VALID_XMP), true);
        let (mut document, mut inspections) =
            PdfDocument::from_bytes_with_inspections(&bytes, &SafetyLimits::default())
                .expect("parse fixture");
        document.trailer_id = Some(vec![b"last-one".to_vec(), b"last-two".to_vec()]);
        inspections.header.is_linearized = true;
        inspections.header.first_linearized_trailer_id = Some(b"first-onefirst-two".to_vec());
        inspections.header.last_trailer_id = Some(b"last-onelast-two".to_vec());
        let report = validate_document(document, inspections, ValidationProfile::PdfA1b);
        assert_rule(&report, "PDFA1B-LINEARIZED-TRAILER-ID-001");
    }

    #[test]
    fn rejects_data_after_the_last_eof_marker() {
        let mut bytes = fixture(Some(VALID_XMP), true);
        bytes.extend_from_slice(b"unexpected");

        let report = validate_bytes(&bytes, ValidationProfile::PdfA1b, &SafetyLimits::default());
        assert_rule(&report, "PDFA1B-POST-EOF-DATA-001");
    }

    #[test]
    fn rejects_xref_streams() {
        let bytes = fixture(Some(VALID_XMP), true);
        let mut document = Document::load_mem(&bytes).expect("load fixture");
        document.reference_table.cross_reference_type = XrefType::CrossReferenceStream;
        let mut bytes = Vec::new();
        document
            .save_to(&mut bytes)
            .expect("save fixture with xref stream");

        let report = validate_bytes(&bytes, ValidationProfile::PdfA1b, &SafetyLimits::default());
        assert_rule(&report, "PDFA1B-XREF-STREAM-001");
    }

    #[test]
    fn rejects_a_missing_trailer_id() {
        let bytes = fixture(Some(VALID_XMP), true);
        let mut document = Document::load_mem(&bytes).expect("load fixture");
        document.trailer.remove(b"ID");
        let mut bytes = Vec::new();
        document
            .save_to(&mut bytes)
            .expect("save fixture without ID");

        let report = validate_bytes(&bytes, ValidationProfile::PdfA1b, &SafetyLimits::default());
        assert_rule(&report, "PDFA1B-TRAILER-ID-001");
    }

    #[test]
    fn reports_missing_xmp() {
        let report = validate_bytes(
            &fixture(None, true),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_rule(&report, "PDFA1B-METADATA-STRUCTURE-001");
        assert_rule(&report, "PDFA1B-ID-SCHEMA-001");
    }

    #[test]
    fn reports_malformed_xmp() {
        let report = validate_bytes(
            &fixture(Some(b"<rdf:RDF>"), true),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_rule(&report, "PDFA1B-XMP-001");
    }

    #[test]
    fn enforces_catalog_metadata_stream_type_subtype_and_filter() {
        for (dictionary, expected) in [
            (
                dictionary! {"Subtype" => "XML"},
                "PDFA1B-METADATA-STRUCTURE-001",
            ),
            (
                dictionary! {"Type" => "Metadata"},
                "PDFA1B-METADATA-STRUCTURE-001",
            ),
            (
                dictionary! {"Type" => "Other", "Subtype" => "XML"},
                "PDFA1B-METADATA-STRUCTURE-001",
            ),
            (
                dictionary! {"Type" => "Metadata", "Subtype" => "Other"},
                "PDFA1B-METADATA-STRUCTURE-001",
            ),
            (
                dictionary! {
                    "Type" => "Metadata",
                    "Subtype" => "XML",
                    "Filter" => "ASCIIHexDecode",
                },
                "PDFA1B-METADATA-FILTER-001",
            ),
        ] {
            let report = validate_bytes(
                &fixture_with_metadata_dictionary(VALID_XMP, dictionary, None),
                ValidationProfile::PdfA1b,
                &SafetyLimits::default(),
            );
            assert_rule(&report, expected);
        }
    }

    /// Confirmed against veraPDF 1.28.2: a catalog Metadata stream with a
    /// direct null `/Filter` is compliant, matching the same direct-null
    /// convention as every other `containsX` predicate this crate checks.
    #[test]
    fn catalog_metadata_direct_null_filter_is_not_a_filter_violation() {
        let report = validate_bytes(
            &fixture_with_metadata_dictionary(
                VALID_XMP,
                dictionary! {
                    "Type" => "Metadata",
                    "Subtype" => "XML",
                    "Filter" => Object::Null,
                },
                None,
            ),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_no_rule(&report, "PDFA1B-METADATA-FILTER-001");
    }

    #[test]
    fn rejects_missing_and_duplicate_identification_declarations() {
        let missing = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"/>"#;
        let report = validate_bytes(
            &fixture(Some(missing), true),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_rule(&report, "PDFA1B-ID-SCHEMA-001");

        let duplicate = br#"<rdf:RDF
          xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/">
          <rdf:Description pdfaid:part="1" pdfaid:conformance="B"/>
          <rdf:Description pdfaid:part="2" pdfaid:conformance="A"/>
        </rdf:RDF>"#;
        let report = validate_bytes(
            &fixture(Some(duplicate), true),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_rule(&report, "PDFA1B-XMP-001");
        assert_rule(&report, "PDFA1B-ID-SCHEMA-001");
    }

    #[test]
    fn accepts_info_values_with_correct_rdf_alt_and_seq_forms() {
        let report = validate_bytes(
            &fixture_with_metadata_dictionary(
                COMPLETE_XMP,
                dictionary! {"Type" => "Metadata", "Subtype" => "XML"},
                Some(complete_info()),
            ),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        for rule in [
            "PDFA1B-INFO-CREATIONDATE-001",
            "PDFA1B-INFO-TITLE-001",
            "PDFA1B-INFO-AUTHOR-001",
            "PDFA1B-INFO-SUBJECT-001",
            "PDFA1B-INFO-KEYWORDS-001",
            "PDFA1B-INFO-CREATOR-001",
            "PDFA1B-INFO-PRODUCER-001",
            "PDFA1B-INFO-MODDATE-001",
        ] {
            assert_no_rule(&report, rule);
        }
    }

    #[test]
    fn reports_every_info_xmp_mismatch_independently() {
        let cases = [
            ("Title", "PDFA1B-INFO-TITLE-001"),
            ("Author", "PDFA1B-INFO-AUTHOR-001"),
            ("Subject", "PDFA1B-INFO-SUBJECT-001"),
            ("Keywords", "PDFA1B-INFO-KEYWORDS-001"),
            ("Creator", "PDFA1B-INFO-CREATOR-001"),
            ("Producer", "PDFA1B-INFO-PRODUCER-001"),
            ("CreationDate", "PDFA1B-INFO-CREATIONDATE-001"),
            ("ModDate", "PDFA1B-INFO-MODDATE-001"),
        ];
        for (key, rule) in cases {
            let mut info = complete_info();
            info.set(
                key,
                Object::String(b"different".to_vec(), StringFormat::Literal),
            );
            let report = validate_bytes(
                &fixture_with_metadata_dictionary(
                    COMPLETE_XMP,
                    dictionary! {"Type" => "Metadata", "Subtype" => "XML"},
                    Some(info),
                ),
                ValidationProfile::PdfA1b,
                &SafetyLimits::default(),
            );
            assert_rule(&report, rule);
        }
    }

    #[test]
    fn author_requires_exactly_one_seq_entry() {
        let xmp = String::from_utf8(COMPLETE_XMP.to_vec())
            .expect("fixture is UTF-8")
            .replace(
                "<rdf:li>Author</rdf:li>",
                "<rdf:li>Author</rdf:li><rdf:li>Second</rdf:li>",
            );
        let report = validate_bytes(
            &fixture_with_metadata_dictionary(
                xmp.as_bytes(),
                dictionary! {"Type" => "Metadata", "Subtype" => "XML"},
                Some(complete_info()),
            ),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_rule(&report, "PDFA1B-INFO-AUTHOR-001");
    }

    #[test]
    fn missing_output_intent_is_outside_the_pinned_output_intent_predicates() {
        let report = validate_bytes(
            &fixture(Some(VALID_XMP), false),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_no_rule(&report, "PDFA1B-OUTPUTINTENT-001");
        assert_no_rule(&report, "PDFA1B-OUTPUTINTENT-IDENTITY-001");
    }

    #[test]
    fn reports_incorrect_pdfa_declarations() {
        let xmp = String::from_utf8(VALID_XMP.to_vec())
            .expect("fixture is UTF-8")
            .replace("pdfaid:part=\"1\"", "pdfaid:part=\"2\"")
            .replace("pdfaid:conformance=\"B\"", "pdfaid:conformance=\"U\"");
        let report = validate_bytes(
            &fixture(Some(xmp.as_bytes()), true),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_rule(&report, "PDFA1B-ID-PART-001");
        assert_rule(&report, "PDFA1B-ID-CONFORMANCE-001");
    }

    #[test]
    fn accepts_pdfa_1a_declaration_for_pdfa_1b_validation() {
        let xmp = String::from_utf8(VALID_XMP.to_vec())
            .expect("fixture is UTF-8")
            .replace("pdfaid:conformance=\"B\"", "pdfaid:conformance=\"A\"");
        let report = validate_bytes(
            &fixture(Some(xmp.as_bytes()), true),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert!(
            report
                .failures
                .iter()
                .all(|failure| failure.rule_id != "PDFA1B-ID-CONFORMANCE-001")
        );
    }

    #[test]
    fn rejects_lowercase_pdfa_conformance() {
        let xmp = String::from_utf8(VALID_XMP.to_vec())
            .expect("fixture is UTF-8")
            .replace("pdfaid:conformance=\"B\"", "pdfaid:conformance=\"b\"");
        let report = validate_bytes(
            &fixture(Some(xmp.as_bytes()), true),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_rule(&report, "PDFA1B-ID-CONFORMANCE-001");
    }

    #[test]
    fn rejects_malformed_pdf_without_panicking() {
        let report = validate_bytes(
            include_bytes!("../tests/fixtures/malformed.pdf"),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_rule(&report, "PDF-PARSE-001");
        assert!(report.document.is_none());
        assert_eq!(report.exit_code(), 2);
    }

    #[test]
    fn reports_real_encrypted_input_as_conformance_failure() {
        let report = validate_bytes(
            include_bytes!("../tests/fixtures/encrypted.pdf"),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_rule(&report, "PDFA1B-ENCRYPTION-001");
        assert!(
            report
                .failures
                .iter()
                .all(|failure| failure.rule_id != "PDF-PARSE-001")
        );
        assert_eq!(report.exit_code(), 2);
    }

    #[test]
    fn missing_input_is_an_operational_failure() {
        let report = validate_file(
            Path::new("tests/fixtures/definitely-not-present.pdf"),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_rule(&report, "INPUT-IO-001");
        assert_eq!(report.failures[0].category, FailureCategory::Operational);
        assert_eq!(report.exit_code(), 1);
    }

    #[test]
    fn input_size_limit_is_an_operational_failure() {
        let limits = SafetyLimits {
            max_input_size: 1,
            ..SafetyLimits::default()
        };
        let report = validate_bytes(
            include_bytes!("../tests/fixtures/structural.pdf"),
            ValidationProfile::PdfA1b,
            &limits,
        );
        assert_rule(&report, "RESOURCE-LIMIT-001");
        assert_eq!(report.exit_code(), 1);
    }

    #[test]
    fn decoded_stream_size_limit_is_an_operational_failure() {
        let limits = SafetyLimits {
            max_decoded_stream_size: 16,
            ..SafetyLimits::default()
        };
        let report = validate_bytes(
            &fixture(Some(VALID_XMP), true),
            ValidationProfile::PdfA1b,
            &limits,
        );
        assert_rule(&report, "RESOURCE-LIMIT-001");
        assert_eq!(report.exit_code(), 1);
    }

    #[test]
    fn reference_depth_limit_is_an_operational_failure() {
        let limits = SafetyLimits {
            max_reference_depth: 0,
            ..SafetyLimits::default()
        };
        let report = validate_bytes(
            &fixture(Some(VALID_XMP), true),
            ValidationProfile::PdfA1b,
            &limits,
        );
        assert_rule(&report, "RESOURCE-LIMIT-001");
        assert_eq!(report.exit_code(), 1);
    }

    #[test]
    fn direct_root_dictionary_fails_catalog_check() {
        let report = validate_bytes(
            &fixture_with_root(Some(VALID_XMP), true, false),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_rule(&report, "PDFA1B-CATALOG-001");
    }

    #[test]
    fn static_structural_fixture_parses() {
        let report = validate_bytes(
            include_bytes!("../tests/fixtures/structural.pdf"),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert!(
            report.document.is_some(),
            "fixture should parse: {:#?}",
            report.failures
        );
        assert!(
            !report.implemented_checks_passed,
            "fixture intentionally has no XMP"
        );
    }

    fn assert_rule(report: &ValidationReport, rule: &str) {
        assert!(
            report
                .failures
                .iter()
                .any(|failure| failure.rule_id == rule),
            "missing {rule}: {:#?}",
            report.failures
        );
    }

    fn assert_no_rule(report: &ValidationReport, rule: &str) {
        assert!(
            report
                .failures
                .iter()
                .all(|failure| failure.rule_id != rule),
            "unexpected {rule}: {:#?}",
            report.failures
        );
    }

    fn fixture(xmp: Option<&[u8]>, output_intent: bool) -> Vec<u8> {
        fixture_with_root(xmp, output_intent, true)
    }

    fn fixture_with_root(xmp: Option<&[u8]>, output_intent: bool, indirect_root: bool) -> Vec<u8> {
        let mut document = pdf_document();
        let pages_id = document.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => Vec::<Object>::new(),
            "Count" => 0,
        });
        let mut catalog = dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        };
        if let Some(xmp) = xmp {
            let metadata_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "Metadata",
                    "Subtype" => "XML",
                },
                xmp.to_vec(),
            ));
            catalog.set("Metadata", metadata_id);
        }
        if output_intent {
            let intent_id = document.add_object(dictionary! {
                "Type" => "OutputIntent",
                "S" => "GTS_PDFA1",
                "OutputConditionIdentifier" => Object::string_literal("Test"),
            });
            catalog.set("OutputIntents", vec![Object::Reference(intent_id)]);
        }
        if indirect_root {
            let catalog_id = document.add_object(catalog);
            document.trailer.set("Root", catalog_id);
        } else {
            document.trailer.set("Root", Object::Dictionary(catalog));
        }
        let mut bytes = Vec::new();
        document.save_to(&mut bytes).expect("save fixture");
        bytes
    }

    fn fixture_with_metadata_dictionary(
        xmp: &[u8],
        metadata_dictionary: Dictionary,
        info: Option<Dictionary>,
    ) -> Vec<u8> {
        let mut document = pdf_document();
        let pages_id = document.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => Vec::<Object>::new(),
            "Count" => 0,
        });
        let metadata_id = document.add_object(Stream::new(metadata_dictionary, xmp.to_vec()));
        let intent_id = document.add_object(dictionary! {
            "Type" => "OutputIntent",
            "S" => "GTS_PDFA1",
            "OutputConditionIdentifier" => Object::string_literal("Test"),
        });
        let catalog_id = document.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "Metadata" => metadata_id,
            "OutputIntents" => vec![Object::Reference(intent_id)],
        });
        document.trailer.set("Root", catalog_id);
        if let Some(info) = info {
            let info_id = document.add_object(info);
            document.trailer.set("Info", info_id);
        }
        let mut bytes = Vec::new();
        document.save_to(&mut bytes).expect("save fixture");
        bytes
    }

    fn complete_info() -> Dictionary {
        dictionary! {
            "Title" => Object::string_literal("Title"),
            "Author" => Object::string_literal("Author"),
            "Subject" => Object::string_literal("Subject"),
            "Keywords" => Object::string_literal("rust,pdf"),
            "Creator" => Object::string_literal("tool"),
            "Producer" => Object::string_literal("producer"),
            "CreationDate" => Object::string_literal("D:20260727123045+02'00'"),
            "ModDate" => Object::string_literal("D:20260727123045+02'00'"),
        }
    }
}
