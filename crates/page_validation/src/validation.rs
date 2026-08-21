use std::fmt;
use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::error::{PdfError, ValidationError};
use crate::limits::SafetyLimits;
use crate::metadata::{dates_equivalent, xmp_integer_value};
use crate::model::{PdfDocument, PdfObjectId};
use crate::report::{
    FailureCategory, RuleFailure, ValidationCounts, ValidationFailure, ValidationReport,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ValidationProfile {
    #[serde(rename = "a-1b")]
    PdfA1b,
    #[serde(rename = "a-1a")]
    PdfA1a,
    #[serde(rename = "a-2b")]
    PdfA2b,
    #[serde(rename = "a-2a")]
    PdfA2a,
    #[serde(rename = "a-2u")]
    PdfA2u,
    #[serde(rename = "a-3b")]
    PdfA3b,
    #[serde(rename = "a-3a")]
    PdfA3a,
    #[serde(rename = "a-3u")]
    PdfA3u,
    #[serde(rename = "a-4")]
    PdfA4,
    #[serde(rename = "a-4e")]
    PdfA4e,
    #[serde(rename = "a-4f")]
    PdfA4f,
    #[serde(rename = "ua-1")]
    PdfUa1,
    #[serde(rename = "ua-2")]
    PdfUa2,
}

impl fmt::Display for ValidationProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PdfA1b => formatter.write_str("PDF/A-1b"),
            Self::PdfA1a => formatter.write_str("PDF/A-1a"),
            Self::PdfA2b => formatter.write_str("PDF/A-2b"),
            Self::PdfA2a => formatter.write_str("PDF/A-2a"),
            Self::PdfA2u => formatter.write_str("PDF/A-2u"),
            Self::PdfA3b => formatter.write_str("PDF/A-3b"),
            Self::PdfA3a => formatter.write_str("PDF/A-3a"),
            Self::PdfA3u => formatter.write_str("PDF/A-3u"),
            Self::PdfA4 => formatter.write_str("PDF/A-4"),
            Self::PdfA4e => formatter.write_str("PDF/A-4e"),
            Self::PdfA4f => formatter.write_str("PDF/A-4f"),
            Self::PdfUa1 => formatter.write_str("PDF/UA-1"),
            Self::PdfUa2 => formatter.write_str("PDF/UA-2"),
        }
    }
}

impl ValidationProfile {
    pub const fn is_implemented(self) -> bool {
        matches!(
            self,
            Self::PdfA1a
                | Self::PdfA1b
                | Self::PdfA2a
                | Self::PdfA2b
                | Self::PdfA2u
                | Self::PdfA3a
                | Self::PdfA3b
                | Self::PdfA3u
                | Self::PdfUa1
        )
    }

    pub const fn pdfa_part(self) -> Option<u8> {
        match self {
            Self::PdfA1a | Self::PdfA1b => Some(1),
            Self::PdfA2a | Self::PdfA2b | Self::PdfA2u => Some(2),
            Self::PdfA3a | Self::PdfA3b | Self::PdfA3u => Some(3),
            _ => None,
        }
    }

    pub const fn pdfa_conformance(self) -> Option<char> {
        match self {
            Self::PdfA1a | Self::PdfA2a | Self::PdfA3a => Some('A'),
            Self::PdfA1b | Self::PdfA2b | Self::PdfA3b => Some('B'),
            Self::PdfA2u | Self::PdfA3u => Some('U'),
            _ => None,
        }
    }

    const fn is_pdfa_2_or_3(self) -> bool {
        matches!(self.pdfa_part(), Some(2 | 3))
    }

    const fn requires_tagged_structure(self) -> bool {
        matches!(self.pdfa_conformance(), Some('A'))
    }

    const fn requires_unicode_mapping(self) -> bool {
        matches!(self.pdfa_conformance(), Some('A' | 'U'))
    }

    const fn permits_optional_content(self) -> bool {
        self.is_pdfa_2_or_3()
    }

    const fn permits_xref_streams(self) -> bool {
        self.is_pdfa_2_or_3()
    }

    const fn permits_transparency(self) -> bool {
        self.is_pdfa_2_or_3()
    }

    const fn permits_embedded_files(self) -> bool {
        matches!(self.pdfa_part(), Some(2 | 3))
    }

    const fn local_rule_prefix(self, level: char) -> Option<&'static str> {
        match self.pdfa_part() {
            Some(1) => None,
            Some(2) => Some(match level {
                'A' => "PDFA2A",
                'U' => "PDFA2U",
                _ => "PDFA2B",
            }),
            Some(3) => Some(match level {
                'A' => "PDFA3A",
                'U' => "PDFA3U",
                _ => "PDFA3B",
            }),
            _ => None,
        }
    }
}

/// The number of validation rules implemented by [`ValidationProfile::PdfA1b`].
const TOTAL_RULE_COUNT: usize = 134;

fn total_rule_count(profile: ValidationProfile) -> usize {
    match profile {
        ValidationProfile::PdfA1b => TOTAL_RULE_COUNT,
        ValidationProfile::PdfA1a => TOTAL_RULE_COUNT + 6,
        ValidationProfile::PdfA2b => 144,
        ValidationProfile::PdfA2a => 154,
        ValidationProfile::PdfA2u => 146,
        ValidationProfile::PdfA3b => 146,
        ValidationProfile::PdfA3a => 156,
        ValidationProfile::PdfA3u => 148,
        ValidationProfile::PdfUa1 => 64,
        _ => 0,
    }
}

/// Returns the sole element of a slice known to hold at most one entry, or
/// `None` for zero or multiple entries.
fn only<T>(items: &[T]) -> Option<&T> {
    (items.len() == 1).then(|| &items[0])
}

/// Validates a file against the profile declared in its XMP metadata.
pub fn validate_file(
    path: &Path,
    limits: &SafetyLimits,
) -> Result<ValidationReport, ValidationError> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > limits.max_input_size {
        return Err(PdfError::InputTooLarge {
            actual: metadata.len(),
            limit: limits.max_input_size,
        }
        .into());
    }

    let bytes = fs::read(path)?;
    validate_bytes(&bytes, limits).map(|report| report.with_source(path))
}

/// Validates a file against an explicitly selected profile.
pub fn validate_file_with_profile(
    path: &Path,
    profile: ValidationProfile,
    limits: &SafetyLimits,
) -> ValidationReport {
    if let Some(report) = validate_profile(profile) {
        return report.with_source(path);
    }

    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => {
            return ValidationReport::operational_failure(
                profile,
                "INPUT-IO-001",
                error.to_string(),
            )
            .with_source(path);
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
        )
        .with_source(path);
    }

    let report = match fs::read(path) {
        Ok(bytes) => validate_bytes_with_profile(&bytes, profile, limits),
        Err(error) => {
            ValidationReport::operational_failure(profile, "INPUT-IO-001", error.to_string())
        }
    };
    report.with_source(path)
}

/// Validates PDF bytes against the profile declared in their XMP metadata.
pub fn validate_bytes(
    bytes: &[u8],
    limits: &SafetyLimits,
) -> Result<ValidationReport, ValidationError> {
    let (document, inspections) = PdfDocument::from_bytes_with_inspections(bytes, limits)?;
    let profile = declared_profile(&document)?;
    if !profile.is_implemented() {
        return Err(ValidationError::UnsupportedProfile(profile));
    }
    Ok(validate_document(document, inspections, profile))
}

/// Validates PDF bytes against an explicitly selected profile.
pub fn validate_bytes_with_profile(
    bytes: &[u8],
    profile: ValidationProfile,
    limits: &SafetyLimits,
) -> ValidationReport {
    if let Some(report) = validate_profile(profile) {
        return report;
    }

    let (document, inspections) = match PdfDocument::from_bytes_with_inspections(bytes, limits) {
        Ok(document) => document,
        Err(error) if error.is_safety_limit() => {
            return ValidationReport::operational_failure(
                profile,
                "RESOURCE-LIMIT-001",
                error.to_string(),
            );
        }
        Err(PdfError::TooManyIndirectObjects { actual, limit }) => {
            return ValidationReport::conformance_failure(
                profile,
                "PDFA1B-INDIRECT-OBJECT-COUNT-001",
                format!(
                    "the document contains {actual} indirect objects, exceeding the PDF/A-1 limit of {limit}"
                ),
            );
        }
        Err(error) => return ValidationReport::parse_failure(profile, error.to_string()),
    };
    validate_document(document, inspections, profile)
}

fn declared_profile(document: &PdfDocument) -> Result<ValidationProfile, ValidationError> {
    if let Some(error) = &document.xmp_parse_error {
        return Err(ValidationError::InvalidProfileDeclaration(format!(
            "XMP metadata could not be parsed: {error}"
        )));
    }
    let Some(xmp) = &document.xmp else {
        return Err(ValidationError::MissingProfileDeclaration);
    };

    match (
        xmp.pdfa_identification_present,
        xmp.pdfua_identification_present,
    ) {
        (false, false) => Err(ValidationError::MissingProfileDeclaration),
        (true, true) => Err(ValidationError::InvalidProfileDeclaration(
            "both PDF/A and PDF/UA identification schemas are present".to_owned(),
        )),
        (true, false) => declared_pdfa_profile(xmp),
        (false, true) => declared_pdfua_profile(xmp),
    }
}

fn declared_pdfa_profile(
    xmp: &crate::metadata::XmpMetadata,
) -> Result<ValidationProfile, ValidationError> {
    let [part] = xmp.pdfa_parts.as_slice() else {
        return Err(ValidationError::InvalidProfileDeclaration(format!(
            "expected exactly one pdfaid:part value, found {:?}",
            xmp.pdfa_parts
        )));
    };
    let part = xmp_integer_value(part).ok_or_else(|| {
        ValidationError::InvalidProfileDeclaration(format!(
            "pdfaid:part value {:?} is not an integer",
            xmp.pdfa_parts[0]
        ))
    })?;
    let conformance = match xmp.pdfa_conformances.as_slice() {
        [] => None,
        [conformance] => Some(conformance.as_str()),
        values => {
            return Err(ValidationError::InvalidProfileDeclaration(format!(
                "expected at most one pdfaid:conformance value, found {values:?}"
            )));
        }
    };

    match (part, conformance) {
        (1, Some("A")) => Ok(ValidationProfile::PdfA1a),
        (1, Some("B")) => Ok(ValidationProfile::PdfA1b),
        (2, Some("A")) => Ok(ValidationProfile::PdfA2a),
        (2, Some("B")) => Ok(ValidationProfile::PdfA2b),
        (2, Some("U")) => Ok(ValidationProfile::PdfA2u),
        (3, Some("A")) => Ok(ValidationProfile::PdfA3a),
        (3, Some("B")) => Ok(ValidationProfile::PdfA3b),
        (3, Some("U")) => Ok(ValidationProfile::PdfA3u),
        (4, None) => Ok(ValidationProfile::PdfA4),
        (4, Some("E")) => Ok(ValidationProfile::PdfA4e),
        (4, Some("F")) => Ok(ValidationProfile::PdfA4f),
        _ => Err(ValidationError::InvalidProfileDeclaration(format!(
            "pdfaid:part {part} and pdfaid:conformance {conformance:?} do not identify a known PDF/A profile"
        ))),
    }
}

fn declared_pdfua_profile(
    xmp: &crate::metadata::XmpMetadata,
) -> Result<ValidationProfile, ValidationError> {
    let [part] = xmp.pdfua_parts.as_slice() else {
        return Err(ValidationError::InvalidProfileDeclaration(format!(
            "expected exactly one pdfuaid:part value, found {:?}",
            xmp.pdfua_parts
        )));
    };
    match xmp_integer_value(part) {
        Some(1) => Ok(ValidationProfile::PdfUa1),
        Some(2) => Ok(ValidationProfile::PdfUa2),
        Some(part) => Err(ValidationError::InvalidProfileDeclaration(format!(
            "pdfuaid:part {part} does not identify a known PDF/UA profile"
        ))),
        None => Err(ValidationError::InvalidProfileDeclaration(format!(
            "pdfuaid:part value {part:?} is not an integer"
        ))),
    }
}

fn validate_profile(profile: ValidationProfile) -> Option<ValidationReport> {
    if profile.is_implemented() {
        None
    } else {
        Some(ValidationReport::operational_failure(
            profile,
            "PROFILE-001",
            format!("validation profile {profile} is not implemented yet"),
        ))
    }
}

fn validate_document(
    document: PdfDocument,
    inspections: crate::model::InspectionSummary,
    profile: ValidationProfile,
) -> ValidationReport {
    let mut failures = Vec::new();

    if profile == ValidationProfile::PdfUa1 {
        if !document
            .xmp
            .as_ref()
            .is_some_and(|xmp| xmp.pdfua_identification_present)
        {
            failures.push(failure(
                "PDFUA1-ID-SCHEMA-001",
                "XMP does not contain the PDF/UA Identification schema",
                document.xmp_object,
                FailureCategory::Metadata,
            ));
        }
        if let Some(failure) = require_single_declared_value(
            document.xmp.as_ref().map(|xmp| xmp.pdfua_parts.as_slice()),
            |value| xmp_integer_value(value) == Some(1),
            "PDFUA1-ID-PART-001",
            "PDF/UA part",
            "pdfuaid:part",
            "one value 1",
            document.xmp_object,
        ) {
            failures.push(failure);
        }
        if document
            .xmp
            .as_ref()
            .is_some_and(|xmp| xmp.pdfua_identification_prefix_failed_tests.contains(&3))
        {
            failures.push(failure(
                "PDFUA1-ID-PART-PREFIX-001",
                "the PDF/UA identification part property uses a lexical prefix other than pdfuaid",
                document.xmp_object,
                FailureCategory::Metadata,
            ));
        }
        if document
            .xmp
            .as_ref()
            .is_some_and(|xmp| xmp.pdfua_identification_prefix_failed_tests.contains(&4))
        {
            failures.push(failure(
                "PDFUA1-ID-AMD-PREFIX-001",
                "the PDF/UA identification amd property uses a lexical prefix other than pdfuaid",
                document.xmp_object,
                FailureCategory::Metadata,
            ));
        }
        if document
            .xmp
            .as_ref()
            .is_some_and(|xmp| xmp.pdfua_identification_prefix_failed_tests.contains(&5))
        {
            failures.push(failure(
                "PDFUA1-ID-CORR-PREFIX-001",
                "the PDF/UA identification corr property uses a lexical prefix other than pdfuaid",
                document.xmp_object,
                FailureCategory::Metadata,
            ));
        }
        if !inspections.header.has_valid_pdfa23_header {
            failures.push(failure(
                "PDFUA1-HEADER-001",
                "the file header must match %PDF-1.n with n between 0 and 7",
                None,
                FailureCategory::Conformance,
            ));
        }
        if document.encrypted
            && !document
                .encryption_permissions
                .is_some_and(|permissions| permissions & 512 == 512)
        {
            failures.push(failure(
                "PDFUA1-ENCRYPTION-P-001",
                "an encrypted document must contain an encryption-dictionary /P entry with bit 10 set",
                document.encryption_dictionary_object,
                FailureCategory::Conformance,
            ));
        }
        if !document.catalog_metadata.is_valid() {
            failures.push(failure(
                "PDFUA1-METADATA-STRUCTURE-001",
                "the document catalog Metadata entry must resolve to a stream with /Type /Metadata and /Subtype /XML",
                document.xmp_object,
                FailureCategory::Metadata,
            ));
        }
        if !document
            .xmp
            .as_ref()
            .is_some_and(|xmp| xmp.dc_title_present)
        {
            failures.push(failure(
                "PDFUA1-METADATA-TITLE-001",
                "the catalog Metadata stream must contain a dc:title entry",
                document.xmp_object,
                FailureCategory::Metadata,
            ));
        }
        if !inspections.document_features.catalog_contains_lang
            && document
                .xmp
                .as_ref()
                .is_some_and(|xmp| xmp.lang_alt_without_x_default)
        {
            failures.push(failure(
                "PDFUA1-METADATA-LANGUAGE-001",
                "natural language for document metadata must be determinable from an x-default language alternative or the catalog /Lang",
                document.xmp_object,
                FailureCategory::Metadata,
            ));
        }
        validate_viewer_preferences(&inspections.document_features, &mut failures);
        validate_mark_info(
            &inspections.document_features,
            &mut failures,
            "PDFUA1-TAGGED-DOCUMENT-001",
            "the document catalog MarkInfo dictionary must contain boolean /Marked true",
        );
        validate_suspects(&inspections.document_features, &mut failures);
        validate_struct_tree_root_presence(
            &inspections.document_features,
            &mut failures,
            "PDFUA1-STRUCT-TREE-ROOT-001",
            "the document catalog must contain a StructTreeRoot entry describing the logical structure hierarchy",
        );
        if inspections.document_features.struct_tree_role_map_has_cycle {
            failures.push(failure(
                "PDFUA1-STRUCT-TREE-ROLE-MAP-CYCLE-001",
                "the StructTreeRoot RoleMap must not contain a circular mapping",
                inspections
                    .document_features
                    .struct_tree_root_object_id
                    .or(inspections.document_features.catalog_id),
                FailureCategory::Conformance,
            ));
        }
        if inspections.document_features.struct_tree_has_unmapped_type {
            failures.push(failure(
                "PDFUA1-STRUCT-TREE-ROLE-MAP-001",
                "every non-standard structure type must resolve through RoleMap to a standard structure type",
                inspections
                    .document_features
                    .struct_tree_root_object_id
                    .or(inspections.document_features.catalog_id),
                FailureCategory::Conformance,
            ));
        }
        if inspections
            .document_features
            .struct_tree_role_map_has_standard_remap
        {
            failures.push(failure(
                "PDFUA1-STRUCT-TREE-ROLE-MAP-STANDARD-001",
                "a standard structure type must not be remapped",
                inspections
                    .document_features
                    .struct_tree_root_object_id
                    .or(inspections.document_features.catalog_id),
                FailureCategory::Conformance,
            ));
        }
        aggregate_failures_with_location(
            &inspections
                .document_features
                .structure_elements_missing_parent,
            "PDFUA1-STRUCT-ELEMENT-PARENT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .toci_elements_not_contained_in_toc,
            "PDFUA1-TOCI-PARENT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .tr_elements_not_contained_in_table_section,
            "PDFUA1-TR-PARENT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .li_elements_not_contained_in_list,
            "PDFUA1-LI-PARENT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .lbody_elements_not_contained_in_li,
            "PDFUA1-LBODY-PARENT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .thead_elements_not_contained_in_table,
            "PDFUA1-THEAD-PARENT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .tbody_elements_not_contained_in_table,
            "PDFUA1-TBODY-PARENT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .tfoot_elements_not_contained_in_table,
            "PDFUA1-TFOOT-PARENT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .th_elements_not_contained_in_tr,
            "PDFUA1-TH-PARENT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .td_elements_not_contained_in_tr,
            "PDFUA1-TD-PARENT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .tr_elements_with_invalid_children,
            "PDFUA1-TR-KIDS-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .toc_elements_with_invalid_children,
            "PDFUA1-TOC-KIDS-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .toc_elements_with_caption_not_first,
            "PDFUA1-TOC-CAPTION-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .list_elements_with_caption_not_first,
            "PDFUA1-L-CAPTION-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .list_elements_with_invalid_children,
            "PDFUA1-L-KIDS-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .list_items_with_invalid_children,
            "PDFUA1-LI-KIDS-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .table_elements_with_invalid_children,
            "PDFUA1-TABLE-KIDS-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .thead_elements_with_invalid_children,
            "PDFUA1-THEAD-KIDS-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .tbody_elements_with_invalid_children,
            "PDFUA1-TBODY-KIDS-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .tfoot_elements_with_invalid_children,
            "PDFUA1-TFOOT-KIDS-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .table_elements_with_multiple_captions,
            "PDFUA1-TABLE-CAPTION-COUNT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .table_elements_with_caption_not_first_or_last,
            "PDFUA1-TABLE-CAPTION-POSITION-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .table_elements_with_multiple_theads,
            "PDFUA1-TABLE-THEAD-COUNT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .table_elements_with_multiple_tfoots,
            "PDFUA1-TABLE-TFOOT-COUNT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .table_elements_with_tfoot_without_tbody,
            "PDFUA1-TABLE-TFOOT-TBODY-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .table_elements_with_thead_without_tbody,
            "PDFUA1-TABLE-THEAD-TBODY-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .table_elements_with_unequal_column_row_spans,
            "PDFUA1-TABLE-COLUMN-ROWSPAN-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .table_elements_with_unequal_row_column_spans,
            "PDFUA1-TABLE-ROW-COLUMNSPAN-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.document_features.table_cells_with_intersections,
            "PDFUA1-TABLE-CELL-INTERSECTION-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .table_cells_with_undetermined_headers,
            "PDFUA1-TABLE-HEADERS-SCOPE-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .table_cells_with_undefined_headers,
            "PDFUA1-TABLE-HEADERS-UNDEFINED-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .figure_elements_missing_alternative_text,
            "PDFUA1-FIGURE-ALTERNATIVE-TEXT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .formula_elements_missing_alternative_text,
            "PDFUA1-FORMULA-ALTERNATIVE-TEXT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.document_features.note_elements_missing_id,
            "PDFUA1-NOTE-ID-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .note_elements_with_duplicate_id,
            "PDFUA1-NOTE-ID-UNIQUE-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.document_features.optional_content_missing_names,
            "PDFUA1-OPTIONAL-CONTENT-NAME-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.document_features.optional_content_as_entries,
            "PDFUA1-OPTIONAL-CONTENT-AS-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .file_specs_missing_or_empty_f_or_uf,
            "PDFUA1-FILE-SPEC-F-AND-UF-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.actions.file_specs_missing_or_empty_f_or_uf,
            "PDFUA1-FILE-SPEC-F-AND-UF-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .heading_elements_with_invalid_nesting,
            "PDFUA1-HEADING-NESTING-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .structure_elements_with_multiple_h_children,
            "PDFUA1-HEADING-CHILD-COUNT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .document_features
                .heading_elements_with_h_in_presence_of_hn,
            "PDFUA1-HEADING-STRUCTURE-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.content.artifacts_inside_tagged_content,
            "PDFUA1-ARTIFACT-NESTED-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.content.tagged_content_inside_artifacts,
            "PDFUA1-TAGGED-CONTENT-INSIDE-ARTIFACT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.content.untagged_content,
            "PDFUA1-CONTENT-TAGGING-001",
            None,
            &mut failures,
        );
        let mut pdfua_language_failures = inspections
            .document_features
            .language_failures_pdfua1
            .clone();
        pdfua_language_failures
            .extend(inspections.content.language_failures_pdfua1.iter().cloned());
        aggregate_failures_with_location(
            &pdfua_language_failures,
            "PDFUA1-LANGUAGE-TAG-001",
            None,
            &mut failures,
        );
        if !inspections.document_features.catalog_contains_lang {
            aggregate_failures_with_location(
                &inspections.actions.outline_entries,
                "PDFUA1-OUTLINE-LANGUAGE-001",
                None,
                &mut failures,
            );
            aggregate_failures_with_location(
                &inspections.document_features.actual_text_language_failures,
                "PDFUA1-ACTUAL-TEXT-LANGUAGE-001",
                None,
                &mut failures,
            );
            aggregate_failures_with_location(
                &inspections.document_features.alt_text_language_failures,
                "PDFUA1-ALT-TEXT-LANGUAGE-001",
                None,
                &mut failures,
            );
            aggregate_failures_with_location(
                &inspections
                    .document_features
                    .expansion_text_language_failures,
                "PDFUA1-E-TEXT-LANGUAGE-001",
                None,
                &mut failures,
            );
            aggregate_failures_with_location(
                &inspections.content.span_actual_text_language_failures,
                "PDFUA1-SPAN-ACTUAL-TEXT-LANGUAGE-001",
                None,
                &mut failures,
            );
            aggregate_failures_with_location(
                &inspections.content.span_alt_text_language_failures,
                "PDFUA1-SPAN-ALT-TEXT-LANGUAGE-001",
                None,
                &mut failures,
            );
            aggregate_failures_with_location(
                &inspections.content.span_expansion_text_language_failures,
                "PDFUA1-SPAN-E-TEXT-LANGUAGE-001",
                None,
                &mut failures,
            );
            aggregate_failures_with_location(
                &inspections.content.text_language_failures,
                "PDFUA1-TEXT-LANGUAGE-001",
                None,
                &mut failures,
            );
            aggregate_failures_with_location(
                &inspections.annotations.contents_language_failures,
                "PDFUA1-ANNOTATION-CONTENTS-LANGUAGE-001",
                None,
                &mut failures,
            );
            aggregate_failures_with_location(
                &inspections.forms.tu_language_failures,
                "PDFUA1-FORM-FIELD-TU-LANGUAGE-001",
                None,
                &mut failures,
            );
            aggregate_failures(
                &inspections.forms.dynamic_xfa_forms,
                "PDFUA1-DYNAMIC-XFA-001",
                &mut failures,
            );
        }
        aggregate_failures_with_location(
            &inspections.annotations.annotations_not_nested_in_annot,
            "PDFUA1-ANNOTATION-ANNOT-TAG-001",
            None,
            &mut failures,
        );
        return finish_report(document, profile, failures, total_rule_count(profile));
    }

    if document.encrypted {
        failures.push(failure(
            "PDFA1B-ENCRYPTION-001",
            "PDF/A-1b does not permit encryption",
            None,
            FailureCategory::Conformance,
        ));
        if document.encrypted_content_unavailable {
            return finish_report(document, profile, failures, 2);
        }
    }
    validate_header(profile, &inspections.header, &mut failures);
    let has_trailer_id = if profile.is_pdfa_2_or_3() {
        if inspections.header.is_linearized {
            inspections
                .header
                .first_linearized_trailer_id
                .as_ref()
                .is_some_and(|id| !id.is_empty())
        } else {
            inspections
                .header
                .last_trailer_id
                .as_ref()
                .is_some_and(|id| !id.is_empty())
        }
    } else if inspections.header.is_linearized {
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
    if !profile.is_pdfa_2_or_3()
        && inspections.header.is_linearized
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
    if profile.is_pdfa_2_or_3() && xmp.is_some_and(|xmp| !xmp.actual_encoding_is_utf8) {
        failures.push(failure(
            "PDFA1B-XMP-ENCODING-001",
            "the XMP package must be encoded as UTF-8",
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
        if profile.is_pdfa_2_or_3()
            && (!xmp.invalid_predefined_xmp_properties.is_empty()
                || !xmp.undefined_extension_xmp_properties.is_empty())
        {
            failures.push(failure(
                "PDFA1B-XMP-PROPERTY-DEFINITION-001",
                "XMP contains a property not defined by a predefined or declared extension schema",
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
            if *test == 7 && !profile.is_pdfa_2_or_3() {
                continue;
            }
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
        let expected_part = match profile.pdfa_part() {
            Some(1) => "one value 1",
            Some(2) => "one value 2",
            Some(3) => "one value 3",
            _ => "the selected PDF/A part",
        };
        if let Some(failure) = require_single_declared_value(
            xmp.map(|xmp| xmp.pdfa_parts.as_slice()),
            |value| xmp_integer_value(value) == profile.pdfa_part().map(i32::from),
            "PDFA1B-ID-PART-001",
            "PDF/A part",
            "pdfaid:part",
            expected_part,
            document.xmp_object,
        ) {
            failures.push(failure);
        }

        let (conformance_rule, expected_conformance) = match profile {
            ValidationProfile::PdfA1a => ("PDFA1A-ID-CONFORMANCE-001", "one A"),
            ValidationProfile::PdfA2a | ValidationProfile::PdfA3a => {
                ("PDFA1A-ID-CONFORMANCE-001", "one A")
            }
            ValidationProfile::PdfA2u | ValidationProfile::PdfA3u => {
                ("PDFA1B-ID-CONFORMANCE-001", "one U")
            }
            _ => ("PDFA1B-ID-CONFORMANCE-001", "one B"),
        };
        if let Some(failure) = require_single_declared_value(
            xmp.map(|xmp| xmp.pdfa_conformances.as_slice()),
            |value| match profile {
                ValidationProfile::PdfA1a => value == "A",
                ValidationProfile::PdfA2a | ValidationProfile::PdfA3a => value == "A",
                ValidationProfile::PdfA2u | ValidationProfile::PdfA3u => value == "U",
                ValidationProfile::PdfA2b | ValidationProfile::PdfA3b => value == "B",
                _ => matches!(value, "A" | "B"),
            },
            conformance_rule,
            "PDF/A conformance",
            "pdfaid:conformance",
            expected_conformance,
            document.xmp_object,
        ) {
            failures.push(failure);
        }
    }

    if !profile.is_pdfa_2_or_3() {
        validate_info_consistency(&document, &mut failures);
    }

    if profile.requires_tagged_structure() {
        validate_tagged_document(&inspections.document_features, &mut failures);
        validate_structure_tree(&inspections.document_features, &mut failures);
        let document_language_failures = if profile.is_pdfa_2_or_3() {
            &inspections.document_features.language_failures_pdfa23
        } else {
            &inspections.document_features.language_failures
        };
        let content_language_failures = if profile.is_pdfa_2_or_3() {
            &inspections.content.language_failures_pdfa23
        } else {
            &inspections.content.language_failures
        };
        aggregate_failures_with_location(
            document_language_failures,
            "PDFA1A-LANG-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            content_language_failures,
            "PDFA1A-LANG-001",
            None,
            &mut failures,
        );
    }

    validate_output_intents(profile, &document, &mut failures);

    aggregate_failures_with_location(
        if profile.is_pdfa_2_or_3() {
            &inspections.icc_based.failures_pdfa2
        } else {
            &inspections.icc_based.failures
        },
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
        if profile.is_pdfa_2_or_3() {
            &inspections.icc_based.invalid_devicen_components_pdfa2
        } else {
            &inspections.icc_based.invalid_devicen_components
        },
        "PDFA1B-DEVICEN-COMPONENTS-001",
        Some("direct DeviceN space"),
        &mut failures,
    );
    if profile.is_pdfa_2_or_3() {
        aggregate_failures_with_location(
            &inspections.icc_based.invalid_devicen_colorants,
            "PDFA1B-DEVICEN-COLORANTS-001",
            Some("direct DeviceN space"),
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.icc_based.inconsistent_separations,
            "PDFA1B-SEPARATION-CONSISTENCY-001",
            None,
            &mut failures,
        );
    }
    let output_color_space = pdfa_output_color_space(&document);
    validate_device_color_spaces(output_color_space, &inspections.icc_based, &mut failures);
    validate_xobjects(profile, &inspections.xobjects, &mut failures);
    validate_graphics(
        profile,
        &inspections.graphics,
        &inspections.content,
        output_color_space,
        &mut failures,
    );
    validate_annotations(
        output_color_space,
        profile,
        &inspections.annotations,
        &mut failures,
    );
    validate_actions(profile, &inspections.actions, &mut failures);
    validate_forms(&inspections.forms, &mut failures);
    validate_document_features(
        profile,
        &inspections.document_features,
        &inspections.actions,
        &mut failures,
    );
    if profile.is_pdfa_2_or_3() {
        let mut invalid_unicode_names = inspections.unicode_names.failures.clone();
        invalid_unicode_names.extend(
            inspections
                .document_features
                .invalid_unicode_structure_types
                .iter()
                .cloned(),
        );
        aggregate_failures(
            &invalid_unicode_names,
            "PDFA1B-UNICODE-NAME-001",
            &mut failures,
        );
    }
    validate_file_specifications(
        profile,
        &inspections.document_features,
        &inspections.actions,
        &mut failures,
    );
    validate_object_limits(
        profile,
        &inspections.object_limits,
        &inspections.content,
        &mut failures,
    );
    validate_stream_safety(
        profile,
        &inspections.stream_safety,
        &inspections.content,
        &mut failures,
    );

    validate_font_dictionaries(profile, &inspections.font_embedding, &mut failures);
    if profile.requires_unicode_mapping() {
        let invalid_unicode_mappings = if profile.is_pdfa_2_or_3() {
            inspections
                .font_embedding
                .invalid_unicode_mappings
                .iter()
                .filter(|failure| {
                    !inspections
                        .font_embedding
                        .unicode_mapping_type3_exemptions
                        .iter()
                        .any(|type3_failure| {
                            type3_failure.object_id == failure.object_id
                                && type3_failure.description == failure.description
                        })
                })
                .cloned()
                .collect::<Vec<_>>()
        } else {
            inspections.font_embedding.invalid_unicode_mappings.clone()
        };
        aggregate_failures(
            &invalid_unicode_mappings,
            "PDFA1A-UNICODE-MAPPING-001",
            &mut failures,
        );
    }
    if profile.is_pdfa_2_or_3() && profile.requires_unicode_mapping() {
        aggregate_failures(
            &inspections.font_embedding.invalid_unicode_values,
            "PDFA1B-UNICODE-VALUE-001",
            &mut failures,
        );
    }
    if matches!(
        profile,
        ValidationProfile::PdfA2a | ValidationProfile::PdfA3a
    ) {
        aggregate_failures(
            &inspections.font_embedding.unicode_pua_without_actual_text,
            "PDFA1A-UNICODE-PUA-ACTUALTEXT-001",
            &mut failures,
        );
    }
    if profile.is_pdfa_2_or_3() {
        aggregate_failures(
            &inspections.font_embedding.notdef_glyphs,
            "PDFA1B-NOTDEF-GLYPH-001",
            &mut failures,
        );
    }
    validate_font_embedding(&inspections.font_embedding, &mut failures);

    finish_report(document, profile, failures, total_rule_count(profile))
}

fn validate_tagged_document(
    features: &crate::document_features::DocumentFeatureSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    validate_mark_info(
        features,
        failures,
        "PDFA1A-TAGGED-DOCUMENT-001",
        "the document catalog MarkInfo dictionary must contain boolean /Marked true",
    );
}

fn validate_mark_info(
    features: &crate::document_features::DocumentFeatureSummary,
    failures: &mut Vec<ValidationFailure>,
    rule_id: &'static str,
    message: &'static str,
) {
    if !features.mark_info_is_dictionary || features.marked != Some(true) {
        failures.push(failure(
            rule_id,
            message,
            features.mark_info_object_id.or(features.catalog_id),
            FailureCategory::Conformance,
        ));
    }
}

fn validate_suspects(
    features: &crate::document_features::DocumentFeatureSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    if features.suspects == Some(true) {
        failures.push(failure(
            "PDFUA1-SUSPECTS-001",
            "the document catalog MarkInfo dictionary must not contain boolean /Suspects true",
            features.mark_info_object_id.or(features.catalog_id),
            FailureCategory::Conformance,
        ));
    }
}

fn validate_viewer_preferences(
    features: &crate::document_features::DocumentFeatureSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    if !features.viewer_preferences_is_dictionary || features.display_doc_title != Some(true) {
        failures.push(failure(
            "PDFUA1-VIEWER-PREFERENCES-001",
            "the document catalog ViewerPreferences dictionary must contain boolean /DisplayDocTitle true",
            features
                .viewer_preferences_object_id
                .or(features.catalog_id),
            FailureCategory::Conformance,
        ));
    }
}

fn validate_structure_tree(
    features: &crate::document_features::DocumentFeatureSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    if !features.struct_tree_root_present || !features.struct_tree_root_valid {
        failures.push(failure(
            "PDFA1A-STRUCT-TREE-ROOT-001",
            "the document catalog must contain a StructTreeRoot entry describing the logical structure hierarchy",
            features.struct_tree_root_object_id.or(features.catalog_id),
            FailureCategory::Conformance,
        ));
    }
    if features.struct_tree_role_map_has_cycle {
        failures.push(failure(
            "PDFA1A-STRUCT-TREE-ROLE-MAP-CYCLE-001",
            "the StructTreeRoot RoleMap must not contain a circular mapping",
            features.struct_tree_root_object_id.or(features.catalog_id),
            FailureCategory::Conformance,
        ));
    }
    if features.struct_tree_has_unmapped_type {
        failures.push(failure(
            "PDFA1A-STRUCT-TREE-ROLE-MAP-001",
            "every non-standard structure type must resolve through RoleMap to a standard structure type",
            features.struct_tree_root_object_id.or(features.catalog_id),
            FailureCategory::Conformance,
        ));
    }
    if features.struct_tree_role_map_has_standard_remap {
        failures.push(failure(
            "PDFA1A-STRUCT-TREE-ROLE-MAP-STANDARD-001",
            "a standard structure type must not be remapped",
            features.struct_tree_root_object_id,
            FailureCategory::Conformance,
        ));
    }
}

fn validate_struct_tree_root_presence(
    features: &crate::document_features::DocumentFeatureSummary,
    failures: &mut Vec<ValidationFailure>,
    rule_id: &'static str,
    message: &'static str,
) {
    if !features.struct_tree_root_present {
        failures.push(failure(
            rule_id,
            message,
            features.struct_tree_root_object_id.or(features.catalog_id),
            FailureCategory::Conformance,
        ));
    }
}

fn validate_header(
    profile: ValidationProfile,
    header: &crate::syntax::HeaderSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    let valid = if profile.is_pdfa_2_or_3() {
        header.has_valid_pdfa23_header
    } else {
        header.has_valid_header
    };
    if !valid {
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
    _profile: ValidationProfile,
    limits: &crate::object_limits::ObjectLimitsSummary,
    content: &crate::content_support::ContentExecutionSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    let checks = [
        (
            "PDFA1B-INTEGER-RANGE-001",
            "an integer is outside the inclusive PDF/A-1 range",
            limits.out_of_range_integers.as_slice(),
        ),
        (
            "PDFA1B-REAL-RANGE-001",
            "a real number is outside the inclusive PDF/A range",
            if _profile.is_pdfa_2_or_3() {
                limits.out_of_range_reals_pdfa_2.as_slice()
            } else {
                limits.out_of_range_reals.as_slice()
            },
        ),
        (
            "PDFA1B-STRING-LENGTH-001",
            "a string exceeds the PDF/A string-length limit",
            if _profile.is_pdfa_2_or_3() {
                limits.overlong_strings_pdfa_2.as_slice()
            } else {
                limits.overlong_strings.as_slice()
            },
        ),
        (
            "PDFA1B-NAME-LENGTH-001",
            "a name exceeds the 127-byte PDF/A-1 limit",
            limits.overlong_names.as_slice(),
        ),
        (
            "PDFA1B-ARRAY-LENGTH-001",
            "an array exceeds the 8,191-entry PDF/A-1 limit",
            if _profile.is_pdfa_2_or_3() {
                &[][..]
            } else {
                limits.oversized_arrays.as_slice()
            },
        ),
        (
            "PDFA1B-DICTIONARY-LENGTH-001",
            "a dictionary exceeds the 4,095-entry PDF/A-1 limit",
            if _profile.is_pdfa_2_or_3() {
                &[][..]
            } else {
                limits.oversized_dictionaries.as_slice()
            },
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
    aggregate_failures_with_location(
        &content.out_of_range_integers,
        "PDFA1B-INTEGER-RANGE-001",
        None,
        failures,
    );
    if !_profile.is_pdfa_2_or_3() {
        aggregate_failures_with_location(
            &content.out_of_range_reals,
            "PDFA1B-REAL-RANGE-001",
            None,
            failures,
        );
        aggregate_failures_with_location(
            &content.overlong_strings,
            "PDFA1B-STRING-LENGTH-001",
            None,
            failures,
        );
    } else {
        aggregate_failures_with_location(
            &content.overlong_strings_pdfa_2,
            "PDFA1B-STRING-LENGTH-001",
            None,
            failures,
        );
    }
    if _profile.is_pdfa_2_or_3() && !limits.underflow_reals_pdfa_2.is_empty() {
        failures.push(failure(
            "PDFA1B-REAL-MINIMUM-001",
            "a real number is nonzero but closer to zero than the PDF/A-2/3 minimum",
            only(&limits.underflow_reals_pdfa_2).copied(),
            FailureCategory::Conformance,
        ));
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
        7 => ("PDFA1B-ID-CORR-PREFIX-001", "corr"),
        _ => unreachable!("unsupported PDF/A-1 identification-prefix test {test}"),
    }
}

fn validate_actions(
    _profile: ValidationProfile,
    actions: &crate::actions::ActionSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    if _profile.is_pdfa_2_or_3() {
        let widget_action_failures = actions
            .widgets_with_actions
            .iter()
            .chain(&actions.widgets_with_additional_actions)
            .cloned()
            .collect::<Vec<_>>();
        aggregate_failures(
            &widget_action_failures,
            "PDFA1B-WIDGET-ACTION-001",
            failures,
        );
    } else {
        aggregate_failures(
            &actions.widgets_with_actions,
            "PDFA1B-WIDGET-ACTION-001",
            failures,
        );
        aggregate_failures(
            &actions.widgets_with_additional_actions,
            "PDFA1B-WIDGET-ADDITIONAL-ACTIONS-001",
            failures,
        );
    }
    for (invalid, rule_id) in [
        (
            if _profile.is_pdfa_2_or_3() {
                actions.invalid_action_types_pdfa2.as_slice()
            } else {
                actions.invalid_action_types.as_slice()
            },
            "PDFA1B-ACTION-TYPE-001",
        ),
        (
            actions.invalid_named_actions.as_slice(),
            "PDFA1B-NAMED-ACTION-001",
        ),
        (
            actions.fields_with_additional_actions.as_slice(),
            "PDFA1B-FIELD-ADDITIONAL-ACTIONS-001",
        ),
        (
            actions.catalog_with_additional_actions.as_slice(),
            "PDFA1B-CATALOG-ADDITIONAL-ACTIONS-001",
        ),
        (
            if _profile.is_pdfa_2_or_3() {
                actions.pages_with_additional_actions.as_slice()
            } else {
                &[][..]
            },
            "PDFA1B-PAGE-ADDITIONAL-ACTIONS-001",
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
    profile: ValidationProfile,
    features: &crate::document_features::DocumentFeatureSummary,
    actions: &crate::actions::ActionSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    for (invalid, rule_id, description) in [
        (
            features.contains_embedded_files_name && !profile.permits_embedded_files(),
            "PDFA1B-NAMES-EMBEDDED-FILES-001",
            "the catalog Names dictionary contains an EmbeddedFiles entry",
        ),
        (
            features.contains_optional_content && !profile.permits_optional_content(),
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
    if profile.is_pdfa_2_or_3() {
        for (invalid, rule_id) in [
            (
                &features.invalid_page_boundaries,
                "PDFA1B-PAGE-BOUNDARY-001",
            ),
            (&features.pages_with_pres_steps, "PDFA1B-PRES-STEPS-001"),
            (
                &features.catalog_with_requirements,
                "PDFA1B-CATALOG-REQUIREMENTS-001",
            ),
            (
                &features.catalog_with_alternate_presentations,
                "PDFA1B-ALTERNATE-PRESENTATIONS-001",
            ),
            (
                &features.catalog_with_needs_rendering,
                "PDFA1B-CATALOG-NEEDS-RENDERING-001",
            ),
            (
                &features.permissions_with_invalid_keys,
                "PDFA1B-PERMS-ENTRIES-001",
            ),
            (&features.acro_forms_with_xfa, "PDFA1B-ACROFORM-XFA-001"),
            (
                &features.optional_content_missing_names,
                "PDFA1B-OPTIONAL-CONTENT-NAME-001",
            ),
            (
                &features.optional_content_duplicate_names,
                "PDFA1B-OPTIONAL-CONTENT-DUPLICATE-NAME-001",
            ),
            (
                &features.optional_content_invalid_orders,
                "PDFA1B-OPTIONAL-CONTENT-ORDER-001",
            ),
            (
                &features.optional_content_as_entries,
                "PDFA1B-OPTIONAL-CONTENT-AS-001",
            ),
            (
                &features.signature_refs_with_digest_keys,
                "PDFA1B-SIGNATURE-REFERENCE-001",
            ),
        ] {
            aggregate_failures(invalid, rule_id, failures);
        }
        aggregate_failures(
            &features.file_specs_missing_f_or_uf,
            "PDFA1B-FILE-SPEC-F-AND-UF-001",
            failures,
        );
        aggregate_failures(
            &actions.file_specs_missing_f_or_uf,
            "PDFA1B-FILE-SPEC-F-AND-UF-001",
            failures,
        );
        if matches!(profile.pdfa_part(), Some(2)) {
            aggregate_failures(
                &features.embedded_files_not_pdfa,
                "PDFA1B-EMBEDDED-FILE-PDFA-001",
                failures,
            );
        }
        if matches!(profile.pdfa_part(), Some(3)) {
            for (invalid, rule_id) in [
                (
                    &features.embedded_files_with_invalid_mime,
                    "PDFA1B-EMBEDDED-FILE-MIME-001",
                ),
                (
                    &features.file_specs_missing_af_relationship,
                    "PDFA1B-FILE-SPEC-AF-RELATIONSHIP-001",
                ),
                (
                    &features.file_specs_not_associated,
                    "PDFA1B-FILE-SPEC-ASSOCIATION-001",
                ),
            ] {
                aggregate_failures(invalid, rule_id, failures);
            }
        }
    }
}

/// Aggregates `PDFA1B-FILE-SPEC-EMBEDDED-FILE-001` failures across every
/// reachability path veraPDF's `CosFileSpecification` object covers: the
/// catalog `Names/EmbeddedFiles` name tree, and `GoToR`/`SubmitForm` action
/// `/F` entries.
fn validate_file_specifications(
    profile: ValidationProfile,
    document_features: &crate::document_features::DocumentFeatureSummary,
    actions: &crate::actions::ActionSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    if profile.is_pdfa_2_or_3() {
        return;
    }
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
    profile: ValidationProfile,
    streams: &crate::stream_safety::StreamSafetySummary,
    content: &crate::content_support::ContentExecutionSummary,
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
    if !profile.is_pdfa_2_or_3() && !streams.lzw_filters.is_empty() {
        let object_id = only(&streams.lzw_filters).copied();
        failures.push(failure(
            "PDFA1B-STREAM-LZW-001",
            "a parsed stream declares the forbidden LZWDecode filter",
            object_id,
            FailureCategory::Conformance,
        ));
    }
    if profile.is_pdfa_2_or_3() && !streams.invalid_filters_pdfa2.is_empty() {
        let object_id = only(&streams.invalid_filters_pdfa2).copied();
        failures.push(failure(
            "PDFA1B-STREAM-FILTER-001",
            "a parsed stream declares a filter that PDF/A-2 and PDF/A-3 do not permit, including LZWDecode",
            object_id,
            FailureCategory::Conformance,
        ));
    }
    if profile.is_pdfa_2_or_3() && !streams.invalid_signature_byte_ranges.is_empty() {
        let object_id = only(&streams.invalid_signature_byte_ranges).copied();
        failures.push(failure(
            "PDFA1B-SIGNATURE-BYTERANGE-001",
            "a signature /ByteRange does not cover the complete PDF except for its signature contents",
            object_id,
            FailureCategory::Conformance,
        ));
    }
    if profile.is_pdfa_2_or_3() && !streams.invalid_signature_certificates.is_empty() {
        let object_id = only(&streams.invalid_signature_certificates).copied();
        failures.push(failure(
            "PDFA1B-SIGNATURE-CERTIFICATE-001",
            "a parsed PKCS#7 signature does not contain a signing certificate",
            object_id,
            FailureCategory::Conformance,
        ));
    }
    if profile.is_pdfa_2_or_3() && !streams.invalid_signature_signer_counts.is_empty() {
        let object_id = only(&streams.invalid_signature_signer_counts).copied();
        failures.push(failure(
            "PDFA1B-SIGNATURE-SIGNER-COUNT-001",
            "a parsed PKCS#7 signature does not contain exactly one signer",
            object_id,
            FailureCategory::Conformance,
        ));
    }
    if profile.is_pdfa_2_or_3()
        && let Some(context) = &content.inline_image_invalid_filter_context
    {
        failures.push(failure(
            "PDFA1B-INLINE-IMAGE-FILTER-001",
            format!("{context} declares a filter that PDF/A-2 and PDF/A-3 do not permit"),
            None,
            FailureCategory::Conformance,
        ));
    }
    if (streams.has_xref_stream || !streams.xref_streams.is_empty())
        && !profile.permits_xref_streams()
    {
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
            streams.has_odd_hex_string || content.has_odd_hex_string,
            "PDFA1B-HEX-STRING-LENGTH-001",
            "a hexadecimal string contains an odd number of non-whitespace characters",
        ),
        (
            streams.has_non_hex_character || content.has_non_hex_character,
            "PDFA1B-HEX-STRING-CHARACTERS-001",
            "a hexadecimal string contains a non-hexadecimal character",
        ),
        (
            streams.has_invalid_xref_subsection_spacing && !profile.is_pdfa_2_or_3(),
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
        if matches!(
            entry.subtype.as_deref(),
            Some("GTS_PDFA1" | "GTS_PDFA2" | "GTS_PDFA3")
        ) {
            output_color_space = entry
                .dest_output_profile_header
                .as_ref()
                .map(|header| header.color_space.as_str());
        }
    }
    output_color_space
}

fn validate_xobjects(
    profile: ValidationProfile,
    xobjects: &crate::xobject::XObjectSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    if profile.is_pdfa_2_or_3() {
        let form_forbidden_entries = xobjects
            .form_opi
            .iter()
            .chain(&xobjects.form_postscript)
            .cloned()
            .collect::<Vec<_>>();
        aggregate_failures(
            &form_forbidden_entries,
            "PDFA1B-FORM-POSTSCRIPT-001",
            failures,
        );
    } else {
        aggregate_failures(&xobjects.form_opi, "PDFA1B-XOBJECT-OPI-001", failures);
        aggregate_failures(
            &xobjects.form_postscript,
            "PDFA1B-FORM-POSTSCRIPT-001",
            failures,
        );
    }
    for (invalid, rule_id) in [
        (
            xobjects.image_alternates.as_slice(),
            "PDFA1B-IMAGE-ALTERNATES-001",
        ),
        (xobjects.image_opi.as_slice(), "PDFA1B-XOBJECT-OPI-001"),
        (
            xobjects.image_interpolate.as_slice(),
            "PDFA1B-IMAGE-INTERPOLATE-001",
        ),
        (
            if profile.is_pdfa_2_or_3() {
                xobjects.image_bits_per_component_pdfa2.as_slice()
            } else {
                xobjects.image_bits_per_component.as_slice()
            },
            "PDFA1B-IMAGE-BPC-001",
        ),
        (
            xobjects.mask_bits_per_component.as_slice(),
            "PDFA1B-IMAGE-MASK-BPC-001",
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
    if profile.is_pdfa_2_or_3() {
        for (index, failures_for_rule) in xobjects.jpeg2000_failures.iter().enumerate() {
            aggregate_failures(
                failures_for_rule,
                match index {
                    0 => "PDFA1B-JPEG2000-CHANNELS-001",
                    1 => "PDFA1B-JPEG2000-COLOR-SPECS-001",
                    2 => "PDFA1B-JPEG2000-COLOR-METHOD-001",
                    3 => "PDFA1B-JPEG2000-COLOR-SPACE-001",
                    4 => "PDFA1B-JPEG2000-BIT-DEPTH-001",
                    _ => unreachable!(),
                },
                failures,
            );
        }
    }
}

fn validate_graphics(
    profile: ValidationProfile,
    graphics: &crate::graphics::GraphicsSummary,
    content: &crate::content_support::ContentExecutionSummary,
    output_color_space: Option<&str>,
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
            if profile.permits_transparency() {
                &[][..]
            } else {
                graphics.extgstate_soft_masks.as_slice()
            },
            "PDFA1B-EXTGSTATE-SMASK-001",
        ),
        (
            if profile.permits_transparency() {
                &[][..]
            } else {
                graphics.xobject_soft_masks.as_slice()
            },
            "PDFA1B-XOBJECT-SMASK-001",
        ),
        (
            if profile.permits_transparency() {
                &[][..]
            } else {
                graphics.transparency_groups.as_slice()
            },
            "PDFA1B-TRANSPARENCY-GROUP-001",
        ),
        (
            if profile.permits_transparency() {
                graphics.blend_modes_pdfa2.as_slice()
            } else {
                graphics.blend_modes.as_slice()
            },
            "PDFA1B-EXTGSTATE-BLEND-MODE-001",
        ),
        (
            if profile.permits_transparency() {
                &[][..]
            } else {
                graphics.stroke_alpha.as_slice()
            },
            "PDFA1B-EXTGSTATE-STROKE-ALPHA-001",
        ),
        (
            if profile.permits_transparency() {
                &[][..]
            } else {
                graphics.fill_alpha.as_slice()
            },
            "PDFA1B-EXTGSTATE-FILL-ALPHA-001",
        ),
    ] {
        aggregate_failures_with_location(invalid, rule_id, None, failures);
    }
    if profile.is_pdfa_2_or_3() {
        if output_color_space.is_none() {
            aggregate_failures_with_location(
                &graphics.transparency_groups_missing_cs,
                "PDFA1B-TRANSPARENCY-GROUP-CS-001",
                None,
                failures,
            );
            aggregate_failures_with_location(
                &graphics.pages_with_transparency_missing_cs,
                "PDFA1B-TRANSPARENCY-GROUP-CS-001",
                None,
                failures,
            );
        }
        for (invalid, rule_id) in [
            (&graphics.extgstate_htp, "PDFA1B-EXTGSTATE-HTP-001"),
            (&graphics.halftone_types, "PDFA1B-HALFTONE-TYPE-001"),
            (&graphics.halftone_names, "PDFA1B-HALFTONE-NAME-001"),
        ] {
            aggregate_failures_with_location(invalid, rule_id, None, failures);
        }
    }
    if profile.is_pdfa_2_or_3() {
        aggregate_failures_with_location(
            &content.icc_cmyk_overprint,
            "PDFA1B-ICCBased-CMYK-OVERPRINT-001",
            None,
            failures,
        );
        aggregate_failures_with_location(
            &content.inherited_resources,
            "PDFA1B-CONTENT-RESOURCES-001",
            None,
            failures,
        );
        aggregate_failures_with_location(
            &graphics.halftone_transfer_functions,
            "PDFA1B-HALFTONE-TRANSFER-FUNCTION-001",
            None,
            failures,
        );
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
    if let Some(context) = &content.inline_image_interpolate_context {
        failures.push(failure(
            "PDFA1B-IMAGE-INTERPOLATE-001",
            format!("{context} sets the forbidden true inline-image interpolation flag"),
            None,
            FailureCategory::Conformance,
        ));
    }
}

fn validate_annotations(
    output_color_space: Option<&str>,
    _profile: ValidationProfile,
    annotations: &crate::annotations::AnnotationSummary,
    failures: &mut Vec<ValidationFailure>,
) {
    for (invalid, rule_id) in [
        (
            if _profile.is_pdfa_2_or_3() {
                annotations.invalid_subtypes_pdfa2.as_slice()
            } else {
                annotations.invalid_subtypes.as_slice()
            },
            "PDFA1B-ANNOTATION-SUBTYPE-001",
        ),
        (
            if _profile.is_pdfa_2_or_3() {
                &[][..]
            } else {
                annotations.invalid_opacities.as_slice()
            },
            "PDFA1B-ANNOTATION-OPACITY-001",
        ),
        (
            if _profile.is_pdfa_2_or_3() {
                annotations.invalid_flags_pdfa2.as_slice()
            } else {
                annotations.invalid_flags.as_slice()
            },
            "PDFA1B-ANNOTATION-FLAGS-001",
        ),
        (
            if _profile.is_pdfa_2_or_3() {
                annotations.missing_flags_pdfa2.as_slice()
            } else {
                &[][..]
            },
            "PDFA1B-ANNOTATION-FLAGS-PRESENT-001",
        ),
        (
            if _profile.is_pdfa_2_or_3() {
                annotations.missing_appearances_pdfa2.as_slice()
            } else {
                &[][..]
            },
            "PDFA1B-ANNOTATION-AP-REQUIRED-001",
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
    if output_color_space != Some("RGB ") && !_profile.is_pdfa_2_or_3() {
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
    _profile: ValidationProfile,
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
            if _profile.is_pdfa_2_or_3() {
                fonts.invalid_font_file_subtypes_pdfa2.as_slice()
            } else {
                fonts.invalid_font_file_subtypes.as_slice()
            },
            "PDFA1B-FONT-FILE-SUBTYPE-001",
        ),
        (
            if _profile.is_pdfa_2_or_3() {
                fonts.incompatible_type0_system_info_pdfa2.as_slice()
            } else {
                fonts.incompatible_type0_system_info.as_slice()
            },
            "PDFA1B-TYPE0-CID-SYSTEM-INFO-001",
        ),
        (
            if _profile.is_pdfa_2_or_3() {
                fonts.invalid_cid_to_gid_maps_pdfa2.as_slice()
            } else {
                fonts.invalid_cid_to_gid_maps.as_slice()
            },
            "PDFA1B-CIDTOGIDMAP-001",
        ),
        (
            if _profile.is_pdfa_2_or_3() {
                fonts.unembedded_cmaps.as_slice()
            } else {
                // PDF/A-1 permits only the two Identity CMaps unembedded;
                // PDF/A-2/3 additionally permit the complete Table 118 set.
                &[]
            },
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
            if _profile.is_pdfa_2_or_3() {
                fonts.invalid_cmap_references.as_slice()
            } else {
                &[]
            },
            "PDFA1B-CMAP-REFERENCE-001",
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
            if _profile.is_pdfa_2_or_3() {
                fonts
                    .invalid_nonsymbolic_truetype_encodings_pdfa2
                    .as_slice()
            } else {
                fonts.invalid_nonsymbolic_truetype_encodings.as_slice()
            },
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
    if !_profile.is_pdfa_2_or_3() {
        aggregate_failures(
            &fonts.missing_cid_subset_cidsets,
            "PDFA1B-CID-SUBSET-CIDSET-001",
            failures,
        );
    }
    if _profile.is_pdfa_2_or_3() {
        aggregate_failures(
            &fonts.invalid_nonsymbolic_truetype_cmaps,
            "PDFA1B-TRUETYPE-NONSYMBOLIC-CMAP-001",
            failures,
        );
    } else {
        aggregate_failures(
            &fonts.invalid_nonsymbolic_truetype_cmaps,
            "PDFA1B-TRUETYPE-NONSYMBOLIC-ENCODING-001",
            failures,
        );
    }
    if !_profile.is_pdfa_2_or_3() {
        aggregate_failures(
            &fonts.unembedded_predefined_cmaps,
            "PDFA1B-CMAP-EMBEDDING-001",
            failures,
        );
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

fn validate_output_intents(
    profile: ValidationProfile,
    document: &PdfDocument,
    failures: &mut Vec<ValidationFailure>,
) {
    validate_output_intent_profiles(profile, document, failures);
    validate_output_intent_identity(document, failures);
    if profile.is_pdfa_2_or_3()
        && document.output_intents_summary.entries.iter().any(|entry| {
            entry.subtype.as_deref() == Some("GTS_PDFX") && entry.dest_output_profile_ref_present
        })
    {
        failures.push(failure(
            "PDFA1B-OUTPUTINTENT-PROFILE-REF-001",
            "a PDF/X OutputIntent contains the forbidden /DestOutputProfileRef entry",
            document.catalog_reference,
            FailureCategory::Conformance,
        ));
    }
}

fn validate_output_intent_profiles(
    profile: ValidationProfile,
    document: &PdfDocument,
    failures: &mut Vec<ValidationFailure>,
) {
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
            .is_some_and(|header| {
                if profile.is_pdfa_2_or_3() {
                    header.conforms_to_pdfa_2_output_intent()
                } else {
                    header.conforms_to_pdfa_1_output_intent()
                }
            })
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
        if !xmp.create_dates.is_empty()
            && !xmp
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
        if !xmp.modify_dates.is_empty()
            && !xmp
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
    for failure in &mut failures {
        failure.rule_id = remap_local_rule_id(profile, &failure.rule_id);
    }
    failures.sort_by_key(|failure| failure.rule_id.clone());
    let failed = failures.len();
    ValidationReport {
        source: None,
        profile,
        checks_passed: failures.is_empty(),
        preliminary: false,
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
        rule_id: rule_id.to_owned(),
        message: message.into(),
        object_id,
        category,
    }
}

fn remap_local_rule_id(profile: ValidationProfile, rule_id: &str) -> String {
    let level = profile.pdfa_conformance().unwrap_or_else(|| {
        if rule_id.starts_with("PDFA1A-") {
            'A'
        } else {
            'B'
        }
    });
    let Some(prefix) = profile.local_rule_prefix(level) else {
        return rule_id.to_owned();
    };
    rule_id
        .strip_prefix("PDFA1A-")
        .or_else(|| rule_id.strip_prefix("PDFA1B-"))
        .map_or_else(|| rule_id.to_owned(), |suffix| format!("{prefix}-{suffix}"))
}

#[cfg(test)]
mod tests {
    use lopdf::xref::XrefType;
    use lopdf::{
        Dictionary, Document, EncryptionState, EncryptionVersion, Object, Permissions, Stream,
        StringFormat, dictionary,
    };

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
        let report = validate_bytes_with_profile(
            &bytes,
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert!(report.checks_passed, "{:#?}", report.failures);
        assert_eq!(report.checks.passed, TOTAL_RULE_COUNT);
    }

    #[test]
    fn infers_the_profile_declared_in_xmp() {
        let bytes = fixture(Some(VALID_XMP), true);
        let report =
            validate_bytes(&bytes, &SafetyLimits::default()).expect("PDF/A-1b profile declaration");

        assert_eq!(report.profile, ValidationProfile::PdfA1b);
        assert!(report.checks_passed, "{:#?}", report.failures);
    }

    #[test]
    fn inferred_validation_requires_a_profile_declaration() {
        let bytes = fixture(None, true);
        let error = validate_bytes(&bytes, &SafetyLimits::default())
            .expect_err("missing profile declaration");

        assert!(matches!(error, ValidationError::MissingProfileDeclaration));
    }

    #[test]
    fn inferred_validation_accepts_implemented_pdfa_1a_declaration() {
        let xmp = std::str::from_utf8(VALID_XMP)
            .expect("fixture is UTF-8")
            .replace("pdfaid:conformance=\"B\"", "pdfaid:conformance=\"A\"");
        let bytes = fixture(Some(xmp.as_bytes()), true);
        let report =
            validate_bytes(&bytes, &SafetyLimits::default()).expect("PDF/A-1a profile declaration");
        assert_eq!(report.profile, ValidationProfile::PdfA1a);
        assert!(report.checks_passed, "{:#?}", report.failures);
        assert_eq!(report.checks.total, TOTAL_RULE_COUNT + 6);
    }

    #[test]
    fn inferred_validation_rejects_an_incomplete_profile_declaration() {
        let xmp = std::str::from_utf8(VALID_XMP)
            .expect("fixture is UTF-8")
            .replace(" pdfaid:conformance=\"B\"", "");
        let bytes = fixture(Some(xmp.as_bytes()), true);
        let error = validate_bytes(&bytes, &SafetyLimits::default())
            .expect_err("incomplete PDF/A-1 declaration");

        assert!(matches!(
            &error,
            ValidationError::InvalidProfileDeclaration(_)
        ));
        assert!(error.to_string().contains("pdfaid:conformance"));
    }

    #[test]
    fn inferred_validation_recognizes_pdfua_declarations() {
        let xmp = br#"<?xpacket begin=""?>
          <x:xmpmeta xmlns:x="adobe:ns:meta/">
            <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
              xmlns:dc="http://purl.org/dc/elements/1.1/">
              <rdf:Description xmlns:pdfuaid="http://www.aiim.org/pdfua/ns/id/"
                pdfuaid:part="1"><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Document title</rdf:li></rdf:Alt></dc:title></rdf:Description>
            </rdf:RDF>
          </x:xmpmeta>
          <?xpacket end="w"?>"#;
        let bytes = fixture(Some(xmp), true);
        let report =
            validate_bytes(&bytes, &SafetyLimits::default()).expect("PDF/UA-1 profile declaration");
        assert_eq!(report.profile, ValidationProfile::PdfUa1);
        assert!(report.checks_passed, "{report:#?}");
        assert_eq!(report.checks.total, 64);
    }

    #[test]
    fn rejects_every_unimplemented_profile_before_parsing() {
        let profiles = [
            ValidationProfile::PdfA4,
            ValidationProfile::PdfA4e,
            ValidationProfile::PdfA4f,
            ValidationProfile::PdfUa2,
        ];

        for profile in profiles {
            let report =
                validate_bytes_with_profile(b"not a PDF", profile, &SafetyLimits::default());
            assert_eq!(report.exit_code(), 1, "profile {profile}");
            assert_eq!(report.failures.len(), 1, "profile {profile}");
            assert_eq!(report.failures[0].rule_id, "PROFILE-001");
            assert_eq!(
                report.failures[0].message,
                format!("validation profile {profile} is not implemented yet")
            );
        }
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

        let report = validate_bytes_with_profile(
            &bytes,
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
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

        let report = validate_bytes_with_profile(
            &bytes,
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
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

        let report = validate_bytes_with_profile(
            &bytes,
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
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

        let report = validate_bytes_with_profile(
            &bytes,
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_rule(&report, "PDFA1B-TRAILER-ID-001");
    }

    #[test]
    fn pdfa_2_and_3_require_a_nonempty_last_trailer_id() {
        let bytes = fixture(Some(VALID_XMP), true);
        for profile in [
            ValidationProfile::PdfA2a,
            ValidationProfile::PdfA2b,
            ValidationProfile::PdfA2u,
            ValidationProfile::PdfA3a,
            ValidationProfile::PdfA3b,
            ValidationProfile::PdfA3u,
        ] {
            let (mut document, mut inspections) =
                PdfDocument::from_bytes_with_inspections(&bytes, &SafetyLimits::default())
                    .expect("parse fixture");
            document.trailer_id = Some(vec![b"parser fallback".to_vec()]);
            inspections.header.last_trailer_id = Some(Vec::new());
            let report = validate_document(document, inspections, profile);
            assert!(
                report
                    .failures
                    .iter()
                    .any(|failure| failure.rule_id.ends_with("-TRAILER-ID-001")),
                "missing profile-specific trailer-ID failure: {report}"
            );
        }
    }

    #[test]
    fn pdfa_2_and_3_require_the_pdfaid_prefix_for_corr() {
        let xmp = br#"<?xpacket begin=""?>
          <x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF
            xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
            xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/"
            xmlns:wrong="http://www.aiim.org/pdfa/ns/id/">
            <rdf:Description pdfaid:part="2" pdfaid:conformance="B"><wrong:corr>1</wrong:corr></rdf:Description>
          </rdf:RDF></x:xmpmeta><?xpacket end="w"?>"#;
        let bytes = fixture(Some(xmp), true);

        let report = validate_bytes_with_profile(
            &bytes,
            ValidationProfile::PdfA2b,
            &SafetyLimits::default(),
        );
        assert_rule(&report, "PDFA2B-ID-CORR-PREFIX-001");

        let report = validate_bytes_with_profile(
            &bytes,
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_no_rule(&report, "PDFA1B-ID-CORR-PREFIX-001");
    }

    #[test]
    fn pdfa_2_and_3_require_utf8_xmp_bytes() {
        let utf16_xmp = VALID_XMP
            .iter()
            .flat_map(|byte| [*byte, 0])
            .collect::<Vec<_>>();
        let report = validate_bytes_with_profile(
            &fixture(Some(&utf16_xmp), true),
            ValidationProfile::PdfA2b,
            &SafetyLimits::default(),
        );

        assert_rule(&report, "PDFA2B-XMP-ENCODING-001");
    }

    #[test]
    fn pdfa_2_and_3_report_lzw_through_the_combined_stream_filter_rule() {
        let bytes = include_bytes!(
            "../tests/fixtures/mutations/PDFA1B-STREAM-LZW-001/shared-document_feature-stream_lzwdecode.pdf"
        );
        let report =
            validate_bytes_with_profile(bytes, ValidationProfile::PdfA2b, &SafetyLimits::default());

        assert_rule(&report, "PDFA2B-STREAM-FILTER-001");
        assert_no_rule(&report, "PDFA2B-STREAM-LZW-001");
    }

    #[test]
    fn pdfa_2_and_3_report_widget_a_and_aa_through_one_rule() {
        let bytes = include_bytes!(
            "../tests/fixtures/mutations/PDFA1B-WIDGET-ADDITIONAL-ACTIONS-001/shared-action-widget_additional_actions.pdf"
        );
        let report =
            validate_bytes_with_profile(bytes, ValidationProfile::PdfA2b, &SafetyLimits::default());

        assert_rule(&report, "PDFA2B-WIDGET-ACTION-001");
        assert_no_rule(&report, "PDFA2B-WIDGET-ADDITIONAL-ACTIONS-001");
    }

    #[test]
    fn reports_missing_xmp() {
        let report = validate_bytes_with_profile(
            &fixture(None, true),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_rule(&report, "PDFA1B-METADATA-STRUCTURE-001");
        assert_rule(&report, "PDFA1B-ID-SCHEMA-001");
    }

    #[test]
    fn reports_malformed_xmp() {
        let report = validate_bytes_with_profile(
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
            let report = validate_bytes_with_profile(
                &fixture_with_metadata_dictionary(VALID_XMP, dictionary, None),
                ValidationProfile::PdfA1b,
                &SafetyLimits::default(),
            );
            assert_rule(&report, expected);
        }
    }

    /// Confirmed against veraPDF 1.30.2: a catalog Metadata stream with a
    /// direct null `/Filter` is compliant, matching the same direct-null
    /// convention as every other `containsX` predicate this crate checks.
    #[test]
    fn catalog_metadata_direct_null_filter_is_not_a_filter_violation() {
        let report = validate_bytes_with_profile(
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
        let report = validate_bytes_with_profile(
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
        let report = validate_bytes_with_profile(
            &fixture(Some(duplicate), true),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_rule(&report, "PDFA1B-XMP-001");
        assert_rule(&report, "PDFA1B-ID-SCHEMA-001");
    }

    #[test]
    fn accepts_info_values_with_correct_rdf_alt_and_seq_forms() {
        let report = validate_bytes_with_profile(
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
            let report = validate_bytes_with_profile(
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
        let report = validate_bytes_with_profile(
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
        let report = validate_bytes_with_profile(
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
        let report = validate_bytes_with_profile(
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
        let report = validate_bytes_with_profile(
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
    fn pdfa_1a_requires_conformance_a() {
        let b = validate_bytes_with_profile(
            &fixture(Some(VALID_XMP), true),
            ValidationProfile::PdfA1a,
            &SafetyLimits::default(),
        );
        assert_rule(&b, "PDFA1A-ID-CONFORMANCE-001");

        let a_xmp = String::from_utf8(VALID_XMP.to_vec())
            .expect("fixture is UTF-8")
            .replace("pdfaid:conformance=\"B\"", "pdfaid:conformance=\"A\"");
        let a = validate_bytes_with_profile(
            &fixture(Some(a_xmp.as_bytes()), true),
            ValidationProfile::PdfA1a,
            &SafetyLimits::default(),
        );
        assert_no_rule(&a, "PDFA1A-ID-CONFORMANCE-001");
        assert_eq!(a.checks.total, TOTAL_RULE_COUNT + 6);
    }

    #[test]
    fn rejects_lowercase_pdfa_conformance() {
        let xmp = String::from_utf8(VALID_XMP.to_vec())
            .expect("fixture is UTF-8")
            .replace("pdfaid:conformance=\"B\"", "pdfaid:conformance=\"b\"");
        let report = validate_bytes_with_profile(
            &fixture(Some(xmp.as_bytes()), true),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_rule(&report, "PDFA1B-ID-CONFORMANCE-001");
    }

    #[test]
    fn rejects_malformed_pdf_without_panicking() {
        let report = validate_bytes_with_profile(
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
        let report = validate_bytes_with_profile(
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
    fn validates_after_decrypting_with_empty_user_password() {
        let mut document =
            Document::load_mem(&fixture(Some(VALID_XMP), true)).expect("load validation fixture");
        let state = EncryptionState::try_from(EncryptionVersion::V1 {
            document: &document,
            owner_password: "owner",
            user_password: "",
            permissions: Permissions::all(),
        })
        .expect("create encryption state");
        document
            .encrypt(&state)
            .expect("encrypt validation fixture");
        let mut bytes = Vec::new();
        document
            .save_to(&mut bytes)
            .expect("save encrypted fixture");

        let report = validate_bytes_with_profile(
            &bytes,
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );

        assert_rule(&report, "PDFA1B-ENCRYPTION-001");
        let document = report.document.as_ref().expect("encrypted PDF is parsed");
        assert!(document.encrypted);
        assert!(!document.encrypted_content_unavailable);
        assert!(document.catalog_present);
        assert!(document.xmp.is_some());
        assert_eq!(report.checks.total, TOTAL_RULE_COUNT);
    }

    #[test]
    fn encryption_takes_precedence_over_object_safety_limit() {
        let limits = SafetyLimits {
            max_object_count: 0,
            ..SafetyLimits::default()
        };
        let report = validate_bytes_with_profile(
            include_bytes!("../tests/fixtures/encrypted.pdf"),
            ValidationProfile::PdfA1b,
            &limits,
        );
        assert_rule(&report, "PDFA1B-ENCRYPTION-001");
        assert!(!report.has_operational_failure());
        assert_eq!(report.exit_code(), 2);
    }

    #[test]
    fn ordinary_object_safety_limit_remains_operational() {
        let limits = SafetyLimits {
            max_object_count: 0,
            ..SafetyLimits::default()
        };
        let report = validate_bytes_with_profile(
            include_bytes!("../tests/fixtures/structural.pdf"),
            ValidationProfile::PdfA1b,
            &limits,
        );
        assert_rule(&report, "RESOURCE-LIMIT-001");
        assert!(
            !report
                .failures
                .iter()
                .any(|failure| { failure.rule_id == "PDFA1B-INDIRECT-OBJECT-COUNT-001" })
        );
        assert_eq!(report.exit_code(), 1);
    }

    #[test]
    fn missing_input_is_an_operational_failure() {
        let path = Path::new("tests/fixtures/definitely-not-present.pdf");
        let report =
            validate_file_with_profile(path, ValidationProfile::PdfA1b, &SafetyLimits::default());
        assert_eq!(report.source.as_deref(), Some(path));
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
        let report = validate_bytes_with_profile(
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
        let report = validate_bytes_with_profile(
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
        let report = validate_bytes_with_profile(
            &fixture(Some(VALID_XMP), true),
            ValidationProfile::PdfA1b,
            &limits,
        );
        assert_rule(&report, "RESOURCE-LIMIT-001");
        assert_eq!(report.exit_code(), 1);
    }

    #[test]
    fn direct_root_dictionary_fails_catalog_check() {
        let report = validate_bytes_with_profile(
            &fixture_with_root(Some(VALID_XMP), true, false),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_rule(&report, "PDFA1B-CATALOG-001");
    }

    #[test]
    fn static_structural_fixture_parses() {
        let report = validate_bytes_with_profile(
            include_bytes!("../tests/fixtures/structural.pdf"),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert!(
            report.document.is_some(),
            "fixture should parse: {:#?}",
            report.failures
        );
        assert!(!report.checks_passed, "fixture intentionally has no XMP");
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
            "MarkInfo" => dictionary! { "Marked" => true },
            "ViewerPreferences" => dictionary! { "DisplayDocTitle" => true },
            "StructTreeRoot" => Dictionary::new(),
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
