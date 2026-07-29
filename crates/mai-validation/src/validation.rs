use std::fmt;
use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::limits::SafetyLimits;
use crate::metadata::dates_equivalent;
use crate::model::PdfDocument;
use crate::report::{FailureCategory, ValidationCounts, ValidationFailure, ValidationReport};

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
const TOTAL_RULE_COUNT: usize = 99;

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

    validate_info_consistency(&document, &mut failures);

    validate_output_intents(&document, &mut failures);

    validate_icc_based(&inspections.icc_based, &mut failures);
    validate_icc_based_components(&inspections.icc_based, &mut failures);
    validate_device_color_spaces(&document, &inspections.icc_based, &mut failures);
    validate_xobjects(&inspections.xobjects, &mut failures);
    validate_graphics(&inspections.graphics, &inspections.icc_based, &mut failures);
    validate_annotations(&document, &inspections.annotations, &mut failures);
    validate_actions(&inspections.actions, &mut failures);
    validate_forms(&inspections.forms, &mut failures);
    validate_document_features(&inspections.document_features, &mut failures);

    validate_font_dictionaries(&inspections.font_embedding, &mut failures);
    validate_font_embedding(&inspections.font_embedding, &mut failures);

    finish_report(document, profile, failures, TOTAL_RULE_COUNT)
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
        aggregate_action_failures(invalid, rule_id, failures);
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
        if invalid.is_empty() {
            continue;
        }
        let object_id = (invalid.len() == 1).then(|| invalid[0].object_id).flatten();
        failures.push(failure(
            rule_id,
            invalid
                .iter()
                .map(|form| form.description.as_str())
                .collect::<Vec<_>>()
                .join("; "),
            object_id,
            FailureCategory::Conformance,
        ));
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
    if !features.file_specs_with_embedded_files.is_empty() {
        let object_id = (features.file_specs_with_embedded_files.len() == 1)
            .then(|| features.file_specs_with_embedded_files[0].object_id)
            .flatten();
        failures.push(failure(
            "PDFA1B-FILE-SPEC-EMBEDDED-FILE-001",
            "a file specification in the EmbeddedFiles name tree contains an EF entry",
            object_id,
            FailureCategory::Conformance,
        ));
    }
}

fn aggregate_action_failures(
    invalid: &[crate::actions::ActionFailure],
    rule_id: &'static str,
    failures: &mut Vec<ValidationFailure>,
) {
    if invalid.is_empty() {
        return;
    }
    let object_id = (invalid.len() == 1).then(|| invalid[0].object_id).flatten();
    failures.push(failure(
        rule_id,
        invalid
            .iter()
            .map(|action| action.description.as_str())
            .collect::<Vec<_>>()
            .join("; "),
        object_id,
        FailureCategory::Conformance,
    ));
}

fn validate_icc_based(
    icc_based: &crate::icc_based::IccBasedSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    if icc_based.failures.is_empty() {
        return;
    }
    let object_id = (icc_based.failures.len() == 1)
        .then(|| icc_based.failures[0].object_id)
        .flatten();
    let detail = icc_based
        .failures
        .iter()
        .map(|profile| match profile.object_id {
            Some(object_id) => format!(
                "{} {}: {}",
                object_id.object_number, object_id.generation, profile.description
            ),
            None => format!("direct profile: {}", profile.description),
        })
        .collect::<Vec<_>>()
        .join("; ");
    failures.push(failure(
        "PDFA1B-ICCBASED-001",
        detail,
        object_id,
        FailureCategory::Conformance,
    ));
}

fn validate_icc_based_components(
    icc_based: &crate::icc_based::IccBasedSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    let invalid = &icc_based.component_failures;
    if invalid.is_empty() {
        return;
    }
    let object_id = (invalid.len() == 1).then(|| invalid[0].object_id).flatten();
    let detail = invalid
        .iter()
        .map(|profile| match profile.object_id {
            Some(object_id) => format!(
                "{} {}: {}",
                object_id.object_number, object_id.generation, profile.description
            ),
            None => format!("direct profile: {}", profile.description),
        })
        .collect::<Vec<_>>()
        .join("; ");
    failures.push(failure(
        "PDFA1B-ICCBASED-COMPONENTS-001",
        detail,
        object_id,
        FailureCategory::Conformance,
    ));
}

fn validate_device_color_spaces(
    document: &PdfDocument,
    color_spaces: &crate::icc_based::IccBasedSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    let output_color_space = pdfa_output_color_space(document);
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
    aggregate_xobject_failures(
        &xobjects.image_alternates,
        "PDFA1B-IMAGE-ALTERNATES-001",
        failures,
    );
    aggregate_xobject_failures(&xobjects.xobject_opi, "PDFA1B-XOBJECT-OPI-001", failures);
    aggregate_xobject_failures(
        &xobjects.image_interpolate,
        "PDFA1B-IMAGE-INTERPOLATE-001",
        failures,
    );
    aggregate_xobject_failures(
        &xobjects.image_bits_per_component,
        "PDFA1B-IMAGE-BPC-001",
        failures,
    );
    aggregate_xobject_failures(
        &xobjects.mask_bits_per_component,
        "PDFA1B-IMAGE-MASK-BPC-001",
        failures,
    );
    aggregate_xobject_failures(
        &xobjects.form_postscript,
        "PDFA1B-FORM-POSTSCRIPT-001",
        failures,
    );
    aggregate_xobject_failures(
        &xobjects.form_reference,
        "PDFA1B-FORM-REFERENCE-001",
        failures,
    );
    aggregate_xobject_failures(
        &xobjects.postscript_xobject,
        "PDFA1B-XOBJECT-POSTSCRIPT-001",
        failures,
    );
}

fn aggregate_xobject_failures(
    invalid: &[crate::xobject::XObjectFailure],
    rule_id: &'static str,
    failures: &mut Vec<ValidationFailure>,
) {
    if invalid.is_empty() {
        return;
    }
    let object_id = (invalid.len() == 1).then(|| invalid[0].object_id);
    let detail = invalid
        .iter()
        .map(|xobject| {
            format!(
                "{} {}: {}",
                xobject.object_id.object_number, xobject.object_id.generation, xobject.description
            )
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

fn validate_graphics(
    graphics: &crate::graphics::GraphicsSummary,
    content: &crate::icc_based::IccBasedSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    aggregate_graphics_failures(
        &graphics.transfer_functions,
        "PDFA1B-EXTGSTATE-TR-001",
        failures,
    );
    aggregate_graphics_failures(
        &graphics.transfer_functions_2,
        "PDFA1B-EXTGSTATE-TR2-001",
        failures,
    );
    aggregate_graphics_failures(
        &graphics.rendering_intents,
        "PDFA1B-RENDERING-INTENT-001",
        failures,
    );
    aggregate_graphics_failures(
        &graphics.extgstate_soft_masks,
        "PDFA1B-EXTGSTATE-SMASK-001",
        failures,
    );
    aggregate_graphics_failures(
        &graphics.xobject_soft_masks,
        "PDFA1B-XOBJECT-SMASK-001",
        failures,
    );
    aggregate_graphics_failures(
        &graphics.transparency_groups,
        "PDFA1B-TRANSPARENCY-GROUP-001",
        failures,
    );
    aggregate_graphics_failures(
        &graphics.blend_modes,
        "PDFA1B-EXTGSTATE-BLEND-MODE-001",
        failures,
    );
    aggregate_graphics_failures(
        &graphics.stroke_alpha,
        "PDFA1B-EXTGSTATE-STROKE-ALPHA-001",
        failures,
    );
    aggregate_graphics_failures(
        &graphics.fill_alpha,
        "PDFA1B-EXTGSTATE-FILL-ALPHA-001",
        failures,
    );
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
}

fn aggregate_graphics_failures(
    invalid: &[crate::graphics::GraphicsFailure],
    rule_id: &'static str,
    failures: &mut Vec<ValidationFailure>,
) {
    if invalid.is_empty() {
        return;
    }
    let object_id = (invalid.len() == 1).then(|| invalid[0].object_id).flatten();
    let detail = invalid
        .iter()
        .map(|failure| match failure.object_id {
            Some(object_id) => format!(
                "{} {}: {}",
                object_id.object_number, object_id.generation, failure.description
            ),
            None => failure.description.clone(),
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

fn validate_annotations(
    document: &PdfDocument,
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
        aggregate_annotation_failures(invalid, rule_id, failures);
    }
    if pdfa_output_color_space(document) != Some("RGB ") {
        aggregate_annotation_failures(
            &annotations.color_uses,
            "PDFA1B-ANNOTATION-COLOR-001",
            failures,
        );
    }
}

fn aggregate_annotation_failures(
    invalid: &[crate::annotations::AnnotationFailure],
    rule_id: &'static str,
    failures: &mut Vec<ValidationFailure>,
) {
    if invalid.is_empty() {
        return;
    }
    let object_id = (invalid.len() == 1).then(|| invalid[0].object_id).flatten();
    failures.push(failure(
        rule_id,
        invalid
            .iter()
            .map(|annotation| annotation.description.as_str())
            .collect::<Vec<_>>()
            .join("; "),
        object_id,
        FailureCategory::Conformance,
    ));
}

fn validate_font_embedding(
    font_embedding: &crate::font_embedding::FontEmbeddingSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    let invalid = &font_embedding.failures;
    if invalid.is_empty() {
        return;
    }
    let object_id = (invalid.len() == 1).then(|| invalid[0].object_id).flatten();
    let message = invalid
        .iter()
        .map(|font| font.description.as_str())
        .collect::<Vec<_>>()
        .join("; ");
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
    ] {
        aggregate_font_failures(invalid, rule_id, failures);
    }
}

fn aggregate_font_failures(
    invalid: &[crate::font_embedding::FontEmbeddingFailure],
    rule_id: &'static str,
    failures: &mut Vec<ValidationFailure>,
) {
    if invalid.is_empty() {
        return;
    }
    let object_id = (invalid.len() == 1).then(|| invalid[0].object_id).flatten();
    failures.push(failure(
        rule_id,
        invalid
            .iter()
            .map(|font| font.description.as_str())
            .collect::<Vec<_>>()
            .join("; "),
        object_id,
        FailureCategory::Conformance,
    ));
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
    object_id: Option<crate::PdfObjectId>,
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
        let object_id = (invalid_profiles.len() == 1)
            .then(|| invalid_profiles[0].0)
            .flatten();
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

#[allow(clippy::too_many_arguments)]
fn compare_field(
    document: &PdfDocument,
    info_key: &str,
    xmp_values: &[String],
    rule_id: &'static str,
    xmp_name: &str,
    object_id: Option<crate::PdfObjectId>,
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
        disclaimer: ValidationReport::DISCLAIMER,
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
    object_id: Option<crate::PdfObjectId>,
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

    #[test]
    fn accepts_all_implemented_checks() {
        let bytes = fixture(Some(VALID_XMP), true);
        let report = validate_bytes(&bytes, ValidationProfile::PdfA1b, &SafetyLimits::default());
        assert!(report.implemented_checks_passed, "{:#?}", report.failures);
        assert_eq!(report.checks.passed, TOTAL_RULE_COUNT);
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
        assert_rule(&report, "PDFA1B-ID-PART-001");
        assert_rule(&report, "PDFA1B-ID-CONFORMANCE-001");
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
        let mut document = Document::with_version("1.4");
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
        let mut document = Document::with_version("1.4");
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
