use std::collections::HashSet;
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

/// A PDF/A or PDF/UA conformance level this crate can validate a document against.
///
/// A profile is either declared by a document's own XMP identification schema or selected explicitly by a caller through the optional `profile` argument accepted by [`validate_pdf_bytes`], [`validate_pdf`], and [`is_pdf_compliant`]. Not every profile in this enum is implemented yet; `Self::is_implemented` reports which ones a `ValidationReport`'s `is_compliant` can be trusted for, and `Self::implemented_check_count` reports how many rules currently back that result.
///
/// ## Examples
///
/// ```rs
/// use page_validation::ValidationProfile;
///
/// assert_eq!(ValidationProfile::PdfA1b.to_string(), "PDF/A-1b");
/// assert!(ValidationProfile::PdfA1b.is_implemented());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ValidationProfile {
    #[serde(rename = "1b")]
    PdfA1b,
    #[serde(rename = "1a")]
    PdfA1a,
    #[serde(rename = "2b")]
    PdfA2b,
    #[serde(rename = "2a")]
    PdfA2a,
    #[serde(rename = "2u")]
    PdfA2u,
    #[serde(rename = "3b")]
    PdfA3b,
    #[serde(rename = "3a")]
    PdfA3a,
    #[serde(rename = "3u")]
    PdfA3u,
    #[serde(rename = "4")]
    PdfA4,
    #[serde(rename = "4e")]
    PdfA4e,
    #[serde(rename = "4f")]
    PdfA4f,
    #[serde(rename = "ua1")]
    PdfUa1,
    #[serde(rename = "ua2")]
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
    /// Returns the number of checks implemented for this profile.
    pub const fn implemented_check_count(self) -> usize {
        match self {
            Self::PdfA1b => 134,
            Self::PdfA1a => 140,
            Self::PdfA2b => 144,
            Self::PdfA2a => 154,
            Self::PdfA2u => 146,
            Self::PdfA3b => 146,
            Self::PdfA3a => 156,
            Self::PdfA3u => 148,
            Self::PdfUa1 => 91,
            _ => 0,
        }
    }

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

/// Returns the sole element of a slice known to hold at most one entry, or
/// `None` for zero or multiple entries.
fn only<T>(items: &[T]) -> Option<&T> {
    items.first().filter(|_| items.len() == 1)
}

/// The selected profile and compliance outcome returned by [`is_pdf_compliant_with_profile`].
///
/// `profile` is either the explicitly requested profile or the one inferred
/// from the document's XMP metadata. `is_compliant` is `false` as soon as the
/// validator finds the first failing rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComplianceResult {
    /// The profile used for validation.
    pub profile: ValidationProfile,
    /// Whether every rule checked before completion passed.
    pub is_compliant: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ValidationMode {
    Exhaustive,
    FirstFailure,
}

/// Reads a file from disk and validates it against a selected profile.
///
/// Pass `None` for `profile` to infer the profile from the document's XMP metadata, or `Some(profile)` to validate against that profile regardless of the declaration. This is the file-based counterpart of [`validate_pdf_bytes`]. It enforces `limits.max_input_size` against the file's size before reading it into memory, then delegates to `validate_pdf_bytes`. The returned report has its `source` set to `path`.
///
/// ## Arguments
///
/// - `path` - The PDF file to read and validate.
/// - `profile` - An explicit validation profile, or `None` to infer it from XMP metadata.
/// - `limits` - The resource bounds enforced while reading, parsing, and inspecting the document.
///
/// ## Returns
///
/// A `ValidationReport` describing which implemented checks for the selected profile passed or failed, with `source` set to `path`.
///
/// ## Errors
///
/// Returns `ValidationError::InputIo` if `path` cannot be read or its size cannot be determined, every parser or safety-limit error `validate_pdf_bytes` can return once the file content is available, and a profile-declaration error when `profile` is `None` and XMP does not unambiguously declare an implemented profile.
///
/// ## Examples
///
/// ```rs
/// use std::path::Path;
///
/// use page_validation::{SafetyLimits, validate_pdf};
///
/// let limits = SafetyLimits::default();
/// let report = validate_pdf(Path::new("input.pdf"), None, &limits)?;
/// println!("{report}");
/// # Ok::<(), page_validation::ValidationError>(())
/// ```
pub fn validate_pdf(
    path: &Path,
    profile: Option<ValidationProfile>,
    limits: &SafetyLimits,
) -> Result<ValidationReport, ValidationError> {
    validate_pdf_with_mode(path, profile, limits, ValidationMode::Exhaustive)
}

fn validate_pdf_with_mode(
    path: &Path,
    profile: Option<ValidationProfile>,
    limits: &SafetyLimits,
    mode: ValidationMode,
) -> Result<ValidationReport, ValidationError> {
    reject_unimplemented_profile(profile)?;
    let bytes = read_file(path, limits)?;
    validate_bytes_with_mode(&bytes, profile, limits, mode).map(|report| report.with_source(path))
}

fn read_file(path: &Path, limits: &SafetyLimits) -> Result<Vec<u8>, ValidationError> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > limits.max_input_size {
        return Err(PdfError::InputTooLarge {
            actual: metadata.len(),
            limit: limits.max_input_size,
        }
        .into());
    }
    Ok(fs::read(path)?)
}

/// Validates PDF bytes already in memory against a selected profile.
///
/// Pass `None` for `profile` to infer the profile from the document's XMP Identification schema, or `Some(profile)` to validate against that profile regardless of the declaration.
///
/// ## Arguments
///
/// - `bytes` - The complete PDF file content.
/// - `profile` - An explicit validation profile, or `None` to infer it from XMP metadata.
/// - `limits` - The resource bounds enforced while parsing and inspecting the document.
///
/// ## Returns
///
/// A `ValidationReport` describing which implemented checks for the selected profile passed or failed.
///
/// ## Errors
///
/// Returns `ValidationError::Pdf` if parsing or inspecting the object graph fails or a `SafetyLimits` bound is exceeded, `ValidationError::MissingProfileDeclaration` or `ValidationError::InvalidProfileDeclaration` if `profile` is `None` and XMP does not unambiguously declare a profile, and `ValidationError::UnsupportedProfile` if the selected profile is not implemented yet.
///
/// ## Examples
///
/// ```rs
/// use page_validation::{SafetyLimits, validate_pdf_bytes};
///
/// let limits = SafetyLimits::default();
/// let error = validate_pdf_bytes(b"not a pdf", None, &limits).unwrap_err();
/// println!("{error}");
/// ```
pub fn validate_pdf_bytes(
    bytes: &[u8],
    profile: Option<ValidationProfile>,
    limits: &SafetyLimits,
) -> Result<ValidationReport, ValidationError> {
    validate_bytes_with_mode(bytes, profile, limits, ValidationMode::Exhaustive)
}

fn validate_bytes_with_mode(
    bytes: &[u8],
    profile: Option<ValidationProfile>,
    limits: &SafetyLimits,
    mode: ValidationMode,
) -> Result<ValidationReport, ValidationError> {
    reject_unimplemented_profile(profile)?;
    let (document, inspections) = PdfDocument::from_bytes_with_inspections(bytes, limits)?;
    let profile = profile.map_or_else(|| declared_profile(&document), Ok)?;
    reject_unimplemented_profile(Some(profile))?;
    Ok(validate_document(document, inspections, profile, mode))
}

fn reject_unimplemented_profile(profile: Option<ValidationProfile>) -> Result<(), ValidationError> {
    if let Some(profile) = profile
        && !profile.is_implemented()
    {
        return Err(ValidationError::UnsupportedProfile(profile));
    }
    Ok(())
}

/// Performs fast validation and returns only the compliance outcome.
///
/// The source and profile-selection behavior matches [`validate_pdf`] but stops after the first failing rule.
pub fn is_pdf_compliant(
    path: &Path,
    profile: Option<ValidationProfile>,
    limits: &SafetyLimits,
) -> Result<bool, ValidationError> {
    is_pdf_compliant_with_profile(path, profile, limits).map(|result| result.is_compliant)
}

/// Performs fast validation of bytes and returns only the compliance outcome.
pub fn is_pdf_compliant_bytes(
    bytes: &[u8],
    profile: Option<ValidationProfile>,
    limits: &SafetyLimits,
) -> Result<bool, ValidationError> {
    is_pdf_compliant_bytes_with_profile(bytes, profile, limits).map(|result| result.is_compliant)
}

/// Performs fast validation and returns the selected profile with the compliance outcome.
///
/// This is useful when callers need both the boolean result and the profile inferred from the document.
pub fn is_pdf_compliant_with_profile(
    path: &Path,
    profile: Option<ValidationProfile>,
    limits: &SafetyLimits,
) -> Result<ComplianceResult, ValidationError> {
    reject_unimplemented_profile(profile)?;
    is_pdf_compliant_bytes_with_profile(&read_file(path, limits)?, profile, limits)
}

/// Performs fast validation of bytes and returns the selected profile with the compliance outcome.
pub fn is_pdf_compliant_bytes_with_profile(
    bytes: &[u8],
    profile: Option<ValidationProfile>,
    limits: &SafetyLimits,
) -> Result<ComplianceResult, ValidationError> {
    reject_unimplemented_profile(profile)?;
    let preparation = PdfDocument::prepare_for_validation(bytes, limits)?;
    let profile = profile.map_or_else(|| declared_profile(preparation.document()), Ok)?;
    reject_unimplemented_profile(Some(profile))?;
    let (preparation, syntax) = preparation.with_syntax(bytes, limits)?;
    let preflight_failed = has_preflight_failure(preparation.document(), &syntax.header, profile);
    let (document, inspections) = preparation.into_inspections_with_syntax(
        bytes,
        limits,
        syntax,
        crate::model::InspectionPlan::for_profile(profile),
    )?;
    let report = validate_document(document, inspections, profile, ValidationMode::FirstFailure);
    Ok(ComplianceResult {
        profile,
        is_compliant: !preflight_failed && report.is_compliant,
    })
}

fn has_preflight_failure(
    document: &PdfDocument,
    header: &crate::syntax::HeaderSummary,
    profile: ValidationProfile,
) -> bool {
    if profile == ValidationProfile::PdfUa1 {
        let xmp = document.xmp.as_ref();
        return !xmp.is_some_and(|xmp| xmp.pdfua_identification_present)
            || !xmp.is_some_and(|xmp| {
                matches!(xmp.pdfua_parts.as_slice(), [part] if xmp_integer_value(part) == Some(1))
            })
            || xmp.is_some_and(|xmp| {
                xmp.pdfua_identification_prefix_failed_tests
                    .iter()
                    .any(|test| matches!(test, 3..=5))
            })
            || (document.encrypted
                && !document
                    .encryption_permissions
                    .is_some_and(|permissions| permissions & 512 == 512))
            || !header.has_valid_pdfa23_header
            || !document.catalog_metadata.is_valid()
            || !xmp.is_some_and(|xmp| xmp.dc_title_present);
    }

    let mut header_failures = Vec::new();
    validate_header(profile, header, &mut header_failures);
    if !header_failures.is_empty() {
        return true;
    }
    let has_trailer_id = if profile.is_pdfa_2_or_3() {
        if header.is_linearized {
            header
                .first_linearized_trailer_id
                .as_ref()
                .is_some_and(|id| !id.is_empty())
        } else {
            header
                .last_trailer_id
                .as_ref()
                .is_some_and(|id| !id.is_empty())
        }
    } else if header.is_linearized {
        header.has_first_linearized_trailer_id
    } else {
        header.last_trailer_id.is_some() || document.trailer_id.is_some()
    };
    if !has_trailer_id
        || (!profile.is_pdfa_2_or_3()
            && header.is_linearized
            && header.last_trailer_id.is_some()
            && header.first_linearized_trailer_id != header.last_trailer_id)
    {
        return true;
    }

    if document.encrypted
        || !document.catalog_present
        || !document.catalog_metadata.is_valid()
        || (document.catalog_metadata.is_stream && document.catalog_metadata.has_filter)
        || document.xmp_parse_error.is_some()
    {
        return true;
    }

    let Some(xmp) = document.xmp.as_ref() else {
        return true;
    };
    if xmp.packet_header_has_bytes
        || xmp.packet_header_has_encoding
        || !xmp.pdfa_identification_present
    {
        return true;
    }
    if profile.is_pdfa_2_or_3() && !xmp.actual_encoding_is_utf8 {
        return true;
    }
    if !xmp.invalid_predefined_xmp_properties.is_empty()
        || !xmp.invalid_predefined_xmp_value_types.is_empty()
        || !xmp.undefined_extension_xmp_properties.is_empty()
        || !xmp.invalid_extension_xmp_value_types.is_empty()
        || !xmp.extension_schema_failed_tests.is_empty()
        || xmp
            .identification_prefix_failed_tests
            .iter()
            .any(|test| *test != 7 || profile.is_pdfa_2_or_3())
    {
        return true;
    }
    if !matches!(xmp.pdfa_parts.as_slice(), [part] if xmp_integer_value(part) == profile.pdfa_part().map(i32::from))
    {
        return true;
    }
    let [conformance] = xmp.pdfa_conformances.as_slice() else {
        return true;
    };
    match profile {
        ValidationProfile::PdfA1a | ValidationProfile::PdfA2a | ValidationProfile::PdfA3a => {
            conformance != "A"
        }
        ValidationProfile::PdfA2u | ValidationProfile::PdfA3u => conformance != "U",
        ValidationProfile::PdfA2b | ValidationProfile::PdfA3b => conformance != "B",
        ValidationProfile::PdfA1b => !matches!(conformance.as_str(), "A" | "B"),
        ValidationProfile::PdfA4
        | ValidationProfile::PdfA4e
        | ValidationProfile::PdfA4f
        | ValidationProfile::PdfUa1
        | ValidationProfile::PdfUa2 => true,
    }
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
            part
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

fn validate_document(
    document: PdfDocument,
    inspections: crate::model::InspectionSummary,
    profile: ValidationProfile,
    mode: ValidationMode,
) -> ValidationReport {
    let mut failures = Vec::new();

    macro_rules! finish_on_first_failure {
        () => {
            if mode == ValidationMode::FirstFailure && !failures.is_empty() {
                return finish_report(
                    document,
                    profile,
                    failures,
                    profile.implemented_check_count(),
                );
            }
        };
    }

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
        finish_on_first_failure!();
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
        finish_on_first_failure!();
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
        finish_on_first_failure!();
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
        finish_on_first_failure!();
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
        finish_on_first_failure!();
        if !inspections.header.has_valid_pdfa23_header {
            failures.push(failure(
                "PDFUA1-HEADER-001",
                "the file header must match %PDF-1.n with n between 0 and 7",
                None,
                FailureCategory::Conformance,
            ));
        }
        finish_on_first_failure!();
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
        finish_on_first_failure!();
        if !document.catalog_metadata.is_valid() {
            failures.push(failure(
                "PDFUA1-METADATA-STRUCTURE-001",
                "the document catalog Metadata entry must resolve to a stream with /Type /Metadata and /Subtype /XML",
                document.xmp_object,
                FailureCategory::Metadata,
            ));
        }
        finish_on_first_failure!();
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
        finish_on_first_failure!();
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
        finish_on_first_failure!();
        validate_viewer_preferences(&inspections.document_features, &mut failures);
        finish_on_first_failure!();
        validate_mark_info(
            &inspections.document_features,
            &mut failures,
            "PDFUA1-TAGGED-DOCUMENT-001",
            "the document catalog MarkInfo dictionary must contain boolean /Marked true",
        );
        finish_on_first_failure!();
        validate_suspects(&inspections.document_features, &mut failures);
        finish_on_first_failure!();
        validate_struct_tree_root_presence(
            &inspections.document_features,
            &mut failures,
            "PDFUA1-STRUCT-TREE-ROOT-001",
            "the document catalog must contain a StructTreeRoot entry describing the logical structure hierarchy",
        );
        finish_on_first_failure!();
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
                .form_elements_without_role_with_invalid_children,
            "PDFUA1-FORM-CHILDREN-001",
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
        for (object_id, form) in &inspections.content.form_xobjects {
            if form.contains_mcid && form.references > 1 {
                failures.push(failure(
                    "PDFUA1-FORM-STRUCTURE-001",
                    format!(
                        "Form XObject {object_id:?} contains marked content with MCIDs and is referenced {} times; its semantic parent is not unique",
                        form.references
                    ),
                    Some((*object_id).into()),
                    FailureCategory::Conformance,
                ));
            }
        }
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
        }
        aggregate_failures(
            &inspections.forms.dynamic_xfa_forms,
            "PDFUA1-DYNAMIC-XFA-001",
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.annotations.annotations_not_nested_in_annot,
            "PDFUA1-ANNOTATION-ANNOT-TAG-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.annotations.links_not_nested_in_link,
            "PDFUA1-LINK-LINK-TAG-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.annotations.links_missing_contents,
            "PDFUA1-LINK-CONTENTS-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.annotations.annotations_missing_contents_or_alt,
            "PDFUA1-ANNOTATION-CONTENTS-ALT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.annotations.trapnet_annotations,
            "PDFUA1-TRAPNET-ANNOTATION-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.annotations.printer_mark_annotations,
            "PDFUA1-PRINTER-MARK-ARTIFACT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.annotations.pages_missing_tabs,
            "PDFUA1-PAGE-TABS-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.forms.widgets_missing_tu_or_alt,
            "PDFUA1-FORM-FIELD-TU-ALT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.forms.widgets_not_nested_in_form,
            "PDFUA1-WIDGET-FORM-TAG-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.xobjects.form_reference,
            "PDFUA1-FORM-REFERENCE-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.actions.media_clips_missing_ct,
            "PDFUA1-MEDIA-CLIP-CT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.actions.media_clips_missing_alt,
            "PDFUA1-MEDIA-CLIP-ALT-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections
                .font_embedding
                .incompatible_type0_system_info_pdfua1,
            "PDFUA1-TYPE0-CID-SYSTEM-INFO-001",
            None,
            &mut failures,
        );
        // PDF/UA-1 7.21.4.1-1 shares the content-reached font population and
        // validated font-program recognition used by PDF/A font embedding.
        aggregate_failures_with_location(
            &inspections.font_embedding.failures,
            "PDFUA1-FONT-EMBEDDING-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.font_embedding.invalid_type1_charsets_pdfua1,
            "PDFUA1-FONT-TYPE1-CHARSET-001",
            None,
            &mut failures,
        );
        aggregate_failures_with_location(
            &inspections.font_embedding.invalid_cid_subset_cidsets_pdfua1,
            "PDFUA1-CID-SUBSET-CIDSET-001",
            None,
            &mut failures,
        );
        // Reuse the shared rendered-glyph scanners: mode-3-only text has no
        // shown bytes, while unresolved mappings remain inapplicable like
        // veraPDF's `isGlyphPresent == null` predicate.
        let mut glyph_presence_failures = inspections
            .font_embedding
            .missing_truetype_glyphs
            .iter()
            .chain(&inspections.font_embedding.missing_type1_glyphs)
            .cloned()
            .collect::<Vec<_>>();
        glyph_presence_failures.sort_by(|left, right| {
            left.object_id
                .cmp(&right.object_id)
                .then_with(|| left.description.cmp(&right.description))
        });
        aggregate_failures_with_location(
            &glyph_presence_failures,
            "PDFUA1-FONT-GLYPH-PRESENCE-001",
            None,
            &mut failures,
        );
        // PDF/UA-1 7.21.5-1 shares the rendered embedded-font width scanner
        // with PDF/A. It already applies the veraPDF tolerance of one font
        // unit and covers simple, composite, Type 1, Type 1C, and Type 3
        // font programs.
        aggregate_failures_with_location(
            &inspections.font_embedding.inconsistent_truetype_widths,
            "PDFUA1-FONT-GLYPH-WIDTH-001",
            None,
            &mut failures,
        );
        // PDF/UA-1 7.21.6-1 shares the bounded embedded TrueType cmap
        // summary with the PDF/A-2/3 implementation. It applies the
        // veraPDF predicate exactly: non-symbolic fonts need at least one
        // non-symbol cmap, and need more than one when a Microsoft Symbol
        // cmap is present.
        aggregate_failures_with_location(
            &inspections
                .font_embedding
                .invalid_nonsymbolic_truetype_cmaps,
            "PDFUA1-TRUETYPE-NONSYMBOLIC-CMAP-001",
            None,
            &mut failures,
        );
        // PDF/UA-1 7.21.6-2 reuses the shared simple-font encoding parser and
        // embedded TrueType cmap scanner. Its cmap requirement is stricter
        // than 7.21.6-1: an embedded non-symbolic TrueType font must contain
        // Microsoft Unicode (3,1), regardless of whether Differences exists.
        aggregate_failures_with_location(
            &inspections
                .font_embedding
                .invalid_nonsymbolic_truetype_encodings_pdfua1,
            "PDFUA1-TRUETYPE-NONSYMBOLIC-ENCODING-001",
            None,
            &mut failures,
        );
        // PDF/UA-1 7.21.6-3 reuses the shared TrueType descriptor and
        // dictionary inspection: symbolic TrueType fonts must not contain
        // an /Encoding entry in the font dictionary.
        aggregate_failures_with_location(
            &inspections
                .font_embedding
                .invalid_symbolic_truetype_encodings,
            "PDFUA1-TRUETYPE-SYMBOLIC-ENCODING-001",
            None,
            &mut failures,
        );
        // PDF/UA-1 7.21.6-4 reuses the bounded embedded TrueType cmap
        // summary. The scanner applies the rule only to recognized embedded
        // programs, matching veraPDF's applicability for TrueTypeFontProgram.
        aggregate_failures_with_location(
            &inspections.font_embedding.invalid_symbolic_truetype_cmaps,
            "PDFUA1-TRUETYPE-SYMBOLIC-CMAP-001",
            None,
            &mut failures,
        );
        // PDF/UA-1 7.21.7-1 reuses the shared rendered-glyph Unicode mapper.
        // It applies the same effective Unicode mapping exceptions used by
        // veraPDF for standard simple-font encodings, Type 1 character names,
        // and Adobe character collections.
        aggregate_failures_with_location(
            &inspections.font_embedding.invalid_unicode_mappings,
            "PDFUA1-FONT-UNICODE-MAPPING-001",
            None,
            &mut failures,
        );
        // PDF/UA-1 7.21.7-2 reuses the shared ToUnicode CMap parser and
        // reserved-value scanner. The scanner rejects U+0000, U+FEFF, and
        // U+FFFE for every font usage, including invisible text.
        aggregate_failures_with_location(
            &inspections.font_embedding.invalid_unicode_values_pdfua1,
            "PDFUA1-FONT-UNICODE-VALUE-001",
            None,
            &mut failures,
        );
        // PDF/UA-1 7.21.8-1 reuses the shared glyph-name/CID resolution but
        // retains text-showing operators made invisible by rendering mode 3.
        aggregate_failures_with_location(
            &inspections.font_embedding.notdef_glyphs_pdfua1,
            "PDFUA1-NOTDEF-GLYPH-001",
            None,
            &mut failures,
        );
        // PDF/UA-1 uses the same embedded Type 2 CIDFont population and
        // CIDToGIDMap shape as PDF/A-2 and PDF/A-3, without their rendering
        // mode exemption. Reuse the already broader applicability vector.
        aggregate_failures_with_location(
            &inspections.font_embedding.invalid_cid_to_gid_maps_pdfa2,
            "PDFUA1-CIDTOGIDMAP-001",
            None,
            &mut failures,
        );
        // Reuse the font scanner's PDCMap applicability: it records used
        // Type 0 encoding CMaps that are neither embedded nor in Table 118.
        aggregate_failures_with_location(
            &inspections.font_embedding.unembedded_cmaps,
            "PDFUA1-CMAP-EMBEDDING-001",
            None,
            &mut failures,
        );
        // Reuse the shared embedded-CMap WMode comparison. Its applicability
        // is already limited to the embedded Type 0 CMaps inspected by the
        // font scanner, matching veraPDF's PDF/UA CMapFile population.
        aggregate_failures_with_location(
            &inspections.font_embedding.invalid_cmap_wmodes,
            "PDFUA1-CMAP-WMODE-001",
            None,
            &mut failures,
        );
        // The shared CMap reference inspection follows the existing PDCMap
        // applicability for used Type 0 encoding CMaps and accepts only the
        // Table 118 predefined CMaps, which is also PDF/UA-1 rule 7.21.3.3-3.
        aggregate_failures_with_location(
            &inspections.font_embedding.invalid_cmap_references,
            "PDFUA1-CMAP-REFERENCE-001",
            None,
            &mut failures,
        );
        return finish_report(
            document,
            profile,
            failures,
            profile.implemented_check_count(),
        );
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
    finish_on_first_failure!();
    validate_header(profile, &inspections.header, &mut failures);
    finish_on_first_failure!();
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
    finish_on_first_failure!();
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
    finish_on_first_failure!();
    if !document.catalog_present {
        failures.push(failure(
            "PDFA1B-CATALOG-001",
            "document trailer does not resolve to a Catalog dictionary",
            document.catalog_reference,
            FailureCategory::Conformance,
        ));
    }
    finish_on_first_failure!();

    let metadata = &document.catalog_metadata;
    if !metadata.is_valid() {
        failures.push(failure(
            "PDFA1B-METADATA-STRUCTURE-001",
            "catalog Metadata must resolve to a stream with /Type /Metadata and /Subtype /XML",
            document.xmp_object,
            FailureCategory::Metadata,
        ));
    }
    finish_on_first_failure!();
    if metadata.is_stream && metadata.has_filter {
        failures.push(failure(
            "PDFA1B-METADATA-FILTER-001",
            "the catalog metadata stream dictionary contains a Filter key",
            document.xmp_object,
            FailureCategory::Metadata,
        ));
    }
    finish_on_first_failure!();

    if let Some(error) = &document.xmp_parse_error {
        failures.push(failure(
            "PDFA1B-XMP-001",
            format!("XMP metadata cannot be parsed: {error}"),
            document.xmp_object,
            FailureCategory::Metadata,
        ));
    }
    finish_on_first_failure!();

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
            let Some((rule_id, message)) = extension_schema_rule(*test) else {
                continue;
            };
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
            let Some((rule_id, property)) = identification_prefix_rule(*test) else {
                continue;
            };
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
    finish_on_first_failure!();
    if !xmp.is_some_and(|xmp| xmp.pdfa_identification_present) {
        failures.push(failure(
            "PDFA1B-ID-SCHEMA-001",
            "XMP does not contain the PDF/A Identification schema",
            document.xmp_object,
            FailureCategory::Metadata,
        ));
    }
    finish_on_first_failure!();

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
    finish_on_first_failure!();

    if !profile.is_pdfa_2_or_3() {
        validate_info_consistency(&document, &mut failures);
    }
    finish_on_first_failure!();

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
    finish_on_first_failure!();

    validate_output_intents(profile, &document, &mut failures);
    finish_on_first_failure!();

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
    finish_on_first_failure!();
    let output_color_space = pdfa_output_color_space(&document);
    validate_device_color_spaces(output_color_space, &inspections.icc_based, &mut failures);
    finish_on_first_failure!();
    validate_xobjects(profile, &inspections.xobjects, &mut failures);
    finish_on_first_failure!();
    validate_graphics(
        profile,
        &inspections.graphics,
        &inspections.content,
        output_color_space,
        &mut failures,
    );
    finish_on_first_failure!();
    validate_annotations(
        output_color_space,
        profile,
        &inspections.annotations,
        &mut failures,
    );
    finish_on_first_failure!();
    validate_actions(profile, &inspections.actions, &mut failures);
    finish_on_first_failure!();
    validate_forms(&inspections.forms, &mut failures);
    finish_on_first_failure!();
    validate_document_features(
        profile,
        &inspections.document_features,
        &inspections.actions,
        &mut failures,
    );
    finish_on_first_failure!();
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
    validate_pdf_specifications(
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

    finish_report(
        document,
        profile,
        failures,
        profile.implemented_check_count(),
    )
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

fn extension_schema_rule(test: u8) -> Option<(&'static str, &'static str)> {
    Some(match test {
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
        _ => return None,
    })
}

fn identification_prefix_rule(test: u8) -> Option<(&'static str, &'static str)> {
    Some(match test {
        4 => ("PDFA1B-ID-PART-PREFIX-001", "part"),
        5 => ("PDFA1B-ID-CONFORMANCE-PREFIX-001", "conformance"),
        6 => ("PDFA1B-ID-AMD-PREFIX-001", "amd"),
        7 => ("PDFA1B-ID-CORR-PREFIX-001", "corr"),
        _ => return None,
    })
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
fn validate_pdf_specifications(
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

/// Computes the object id and first distinct description used by every
/// same-rule failure aggregator.
fn joined_failure(invalid: &[RuleFailure]) -> (Option<PdfObjectId>, String) {
    let mut seen = HashSet::new();
    let first = invalid
        .iter()
        .find(|entry| seen.insert(entry.description.as_str()))
        .expect("failure aggregation requires at least one entry");
    let only_failure = invalid
        .iter()
        .all(|entry| entry.description == first.description);
    let object_id = (invalid
        .iter()
        .all(|entry| entry.object_id == first.object_id)
        && only_failure)
        .then_some(first.object_id)
        .flatten();
    (object_id, first.description.clone())
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

/// Like [`aggregate_failures`], but keeps the first distinct failure
/// description and prefixes entries without an object id with `no_id_label`
/// when given.
fn aggregate_failures_with_location(
    invalid: &[RuleFailure],
    rule_id: &'static str,
    no_id_label: Option<&str>,
    failures: &mut Vec<ValidationFailure>,
) {
    if invalid.is_empty() {
        return;
    }
    let mut seen = HashSet::new();
    let first = invalid
        .iter()
        .find(|entry| {
            let no_id_label = entry.object_id.is_none().then_some(no_id_label).flatten();
            seen.insert((no_id_label, entry.description.as_str()))
        })
        .expect("failure aggregation requires at least one entry");
    let no_id_label = first.object_id.is_none().then_some(no_id_label).flatten();
    let detail = match no_id_label {
        Some(label) => format!("{label}: {}", first.description),
        None => first.description.clone(),
    };
    let only_failure = invalid.iter().all(|entry| {
        let entry_no_id_label = entry.object_id.is_none().then_some(no_id_label).flatten();
        entry_no_id_label == no_id_label && entry.description == first.description
    });
    let object_id = (invalid
        .iter()
        .all(|entry| entry.object_id == first.object_id)
        && only_failure)
        .then_some(first.object_id)
        .flatten();
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
                [
                    "PDFA1B-JPEG2000-CHANNELS-001",
                    "PDFA1B-JPEG2000-COLOR-SPECS-001",
                    "PDFA1B-JPEG2000-COLOR-METHOD-001",
                    "PDFA1B-JPEG2000-COLOR-SPACE-001",
                    "PDFA1B-JPEG2000-BIT-DEPTH-001",
                ]
                .get(index)
                .copied()
                .unwrap_or("PDFA1B-JPEG2000-BIT-DEPTH-001"),
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
        is_compliant: failures.is_empty(),
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
        let report = validate_pdf_bytes(
            &bytes,
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert!(report.is_compliant, "{:#?}", report.failures);
        assert_eq!(
            report.checks.passed,
            ValidationProfile::PdfA1b.implemented_check_count()
        );
    }

    #[test]
    fn infers_the_profile_declared_in_xmp() {
        let bytes = fixture(Some(VALID_XMP), true);
        let report = validate_pdf_bytes(&bytes, None, &SafetyLimits::default())
            .expect("PDF/A-1b profile declaration");

        assert_eq!(report.profile, ValidationProfile::PdfA1b);
        assert!(report.is_compliant, "{:#?}", report.failures);
    }

    #[test]
    fn fast_validation_infers_the_profile_and_returns_compliance() {
        let bytes = fixture(Some(VALID_XMP), true);

        let result = is_pdf_compliant_bytes(&bytes, None, &SafetyLimits::default())
            .expect("PDF/A-1b profile declaration");

        assert!(result);
    }

    #[test]
    fn fast_validation_stops_after_the_first_failure() {
        let bytes = fixture(Some(VALID_XMP), true);
        let mut document = Document::load_mem(&bytes).expect("load validation fixture");
        document.trailer.remove(b"ID");
        let mut invalid = Vec::new();
        document
            .save_to(&mut invalid)
            .expect("write invalid fixture");

        let result = is_pdf_compliant_bytes_with_profile(
            &invalid,
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("validate fixture");

        assert_eq!(result.profile, ValidationProfile::PdfA1b);
        assert!(!result.is_compliant);
    }

    #[test]
    fn fast_validation_accepts_file_input() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/trailer-id-missing.pdf");

        let result = is_pdf_compliant(
            &path,
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("validate fixture");

        assert!(!result);
    }

    #[test]
    fn inferred_validation_requires_a_profile_declaration() {
        let bytes = fixture(None, true);
        let error = validate_pdf_bytes(&bytes, None, &SafetyLimits::default())
            .expect_err("missing profile declaration");

        assert!(matches!(error, ValidationError::MissingProfileDeclaration));
    }

    #[test]
    fn inferred_validation_accepts_implemented_pdfa_1a_declaration() {
        let xmp = std::str::from_utf8(VALID_XMP)
            .expect("fixture is UTF-8")
            .replace("pdfaid:conformance=\"B\"", "pdfaid:conformance=\"A\"");
        let bytes = fixture(Some(xmp.as_bytes()), true);
        let report = validate_pdf_bytes(&bytes, None, &SafetyLimits::default())
            .expect("PDF/A-1a profile declaration");
        assert_eq!(report.profile, ValidationProfile::PdfA1a);
        assert!(report.is_compliant, "{:#?}", report.failures);
        assert_eq!(
            report.checks.total,
            ValidationProfile::PdfA1a.implemented_check_count()
        );
    }

    #[test]
    fn inferred_validation_rejects_an_incomplete_profile_declaration() {
        let xmp = std::str::from_utf8(VALID_XMP)
            .expect("fixture is UTF-8")
            .replace(" pdfaid:conformance=\"B\"", "");
        let bytes = fixture(Some(xmp.as_bytes()), true);
        let error = validate_pdf_bytes(&bytes, None, &SafetyLimits::default())
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
        let report = validate_pdf_bytes(&bytes, None, &SafetyLimits::default())
            .expect("PDF/UA-1 profile declaration");
        assert_eq!(report.profile, ValidationProfile::PdfUa1);
        assert!(report.is_compliant, "{report:#?}");
        assert_eq!(
            report.checks.total,
            ValidationProfile::PdfUa1.implemented_check_count()
        );
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
            let error = validate_pdf_bytes(b"not a PDF", Some(profile), &SafetyLimits::default())
                .expect_err("unimplemented profile");
            assert!(
                matches!(error, ValidationError::UnsupportedProfile(actual) if actual == profile)
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

        let report = validate_pdf_bytes(
            &bytes,
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
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
        let report = validate_document(
            document,
            inspections,
            ValidationProfile::PdfA1b,
            ValidationMode::Exhaustive,
        );
        assert_rule(&report, "PDFA1B-LINEARIZED-TRAILER-ID-001");
    }

    #[test]
    fn rejects_data_after_the_last_eof_marker() {
        let mut bytes = fixture(Some(VALID_XMP), true);
        bytes.extend_from_slice(b"unexpected");

        let report = validate_pdf_bytes(
            &bytes,
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
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

        let report = validate_pdf_bytes(
            &bytes,
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
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

        let report = validate_pdf_bytes(
            &bytes,
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
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
            let report =
                validate_document(document, inspections, profile, ValidationMode::Exhaustive);
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

        let report = validate_pdf_bytes(
            &bytes,
            Some(ValidationProfile::PdfA2b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert_rule(&report, "PDFA2B-ID-CORR-PREFIX-001");

        let report = validate_pdf_bytes(
            &bytes,
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert_no_rule(&report, "PDFA1B-ID-CORR-PREFIX-001");
    }

    #[test]
    fn pdfa_2_and_3_require_utf8_xmp_bytes() {
        let utf16_xmp = VALID_XMP
            .iter()
            .flat_map(|byte| [*byte, 0])
            .collect::<Vec<_>>();
        let report = validate_pdf_bytes(
            &fixture(Some(&utf16_xmp), true),
            Some(ValidationProfile::PdfA2b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");

        assert_rule(&report, "PDFA2B-XMP-ENCODING-001");
    }

    #[test]
    fn pdfa_2_and_3_report_lzw_through_the_combined_stream_filter_rule() {
        let bytes = include_bytes!(
            "../tests/fixtures/mutations/PDFA1B-STREAM-LZW-001/shared-document_feature-stream_lzwdecode.pdf"
        );
        let report = validate_pdf_bytes(
            bytes,
            Some(ValidationProfile::PdfA2b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");

        assert_rule(&report, "PDFA2B-STREAM-FILTER-001");
        assert_no_rule(&report, "PDFA2B-STREAM-LZW-001");
    }

    #[test]
    fn pdfa_2_and_3_report_widget_a_and_aa_through_one_rule() {
        let bytes = include_bytes!(
            "../tests/fixtures/mutations/PDFA1B-WIDGET-ADDITIONAL-ACTIONS-001/shared-action-widget_additional_actions.pdf"
        );
        let report = validate_pdf_bytes(
            bytes,
            Some(ValidationProfile::PdfA2b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");

        assert_rule(&report, "PDFA2B-WIDGET-ACTION-001");
        assert_no_rule(&report, "PDFA2B-WIDGET-ADDITIONAL-ACTIONS-001");
    }

    #[test]
    fn reports_missing_xmp() {
        let report = validate_pdf_bytes(
            &fixture(None, true),
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert_rule(&report, "PDFA1B-METADATA-STRUCTURE-001");
        assert_rule(&report, "PDFA1B-ID-SCHEMA-001");
    }

    #[test]
    fn reports_malformed_xmp() {
        let report = validate_pdf_bytes(
            &fixture(Some(b"<rdf:RDF>"), true),
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
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
            let report = validate_pdf_bytes(
                &fixture_with_metadata_dictionary(VALID_XMP, dictionary, None),
                Some(ValidationProfile::PdfA1b),
                &SafetyLimits::default(),
            )
            .expect("explicit profile validation");
            assert_rule(&report, expected);
        }
    }

    /// Confirmed against veraPDF 1.30.2: a catalog Metadata stream with a
    /// direct null `/Filter` is compliant, matching the same direct-null
    /// convention as every other `containsX` predicate this crate checks.
    #[test]
    fn catalog_metadata_direct_null_filter_is_not_a_filter_violation() {
        let report = validate_pdf_bytes(
            &fixture_with_metadata_dictionary(
                VALID_XMP,
                dictionary! {
                    "Type" => "Metadata",
                    "Subtype" => "XML",
                    "Filter" => Object::Null,
                },
                None,
            ),
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert_no_rule(&report, "PDFA1B-METADATA-FILTER-001");
    }

    #[test]
    fn rejects_missing_and_duplicate_identification_declarations() {
        let missing = br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"/>"#;
        let report = validate_pdf_bytes(
            &fixture(Some(missing), true),
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert_rule(&report, "PDFA1B-ID-SCHEMA-001");

        let duplicate = br#"<rdf:RDF
          xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
          xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/">
          <rdf:Description pdfaid:part="1" pdfaid:conformance="B"/>
          <rdf:Description pdfaid:part="2" pdfaid:conformance="A"/>
        </rdf:RDF>"#;
        let report = validate_pdf_bytes(
            &fixture(Some(duplicate), true),
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert_rule(&report, "PDFA1B-XMP-001");
        assert_rule(&report, "PDFA1B-ID-SCHEMA-001");
    }

    #[test]
    fn accepts_info_values_with_correct_rdf_alt_and_seq_forms() {
        let report = validate_pdf_bytes(
            &fixture_with_metadata_dictionary(
                COMPLETE_XMP,
                dictionary! {"Type" => "Metadata", "Subtype" => "XML"},
                Some(complete_info()),
            ),
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
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
            let report = validate_pdf_bytes(
                &fixture_with_metadata_dictionary(
                    COMPLETE_XMP,
                    dictionary! {"Type" => "Metadata", "Subtype" => "XML"},
                    Some(info),
                ),
                Some(ValidationProfile::PdfA1b),
                &SafetyLimits::default(),
            )
            .expect("explicit profile validation");
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
        let report = validate_pdf_bytes(
            &fixture_with_metadata_dictionary(
                xmp.as_bytes(),
                dictionary! {"Type" => "Metadata", "Subtype" => "XML"},
                Some(complete_info()),
            ),
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert_rule(&report, "PDFA1B-INFO-AUTHOR-001");
    }

    #[test]
    fn missing_output_intent_is_outside_the_pinned_output_intent_predicates() {
        let report = validate_pdf_bytes(
            &fixture(Some(VALID_XMP), false),
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert_no_rule(&report, "PDFA1B-OUTPUTINTENT-001");
        assert_no_rule(&report, "PDFA1B-OUTPUTINTENT-IDENTITY-001");
    }

    #[test]
    fn reports_incorrect_pdfa_declarations() {
        let xmp = String::from_utf8(VALID_XMP.to_vec())
            .expect("fixture is UTF-8")
            .replace("pdfaid:part=\"1\"", "pdfaid:part=\"2\"")
            .replace("pdfaid:conformance=\"B\"", "pdfaid:conformance=\"U\"");
        let report = validate_pdf_bytes(
            &fixture(Some(xmp.as_bytes()), true),
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert_rule(&report, "PDFA1B-ID-PART-001");
        assert_rule(&report, "PDFA1B-ID-CONFORMANCE-001");
    }

    #[test]
    fn accepts_pdfa_1a_declaration_for_pdfa_1b_validation() {
        let xmp = String::from_utf8(VALID_XMP.to_vec())
            .expect("fixture is UTF-8")
            .replace("pdfaid:conformance=\"B\"", "pdfaid:conformance=\"A\"");
        let report = validate_pdf_bytes(
            &fixture(Some(xmp.as_bytes()), true),
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert!(
            report
                .failures
                .iter()
                .all(|failure| failure.rule_id != "PDFA1B-ID-CONFORMANCE-001")
        );
    }

    #[test]
    fn pdfa_1a_requires_conformance_a() {
        let b = validate_pdf_bytes(
            &fixture(Some(VALID_XMP), true),
            Some(ValidationProfile::PdfA1a),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert_rule(&b, "PDFA1A-ID-CONFORMANCE-001");

        let a_xmp = String::from_utf8(VALID_XMP.to_vec())
            .expect("fixture is UTF-8")
            .replace("pdfaid:conformance=\"B\"", "pdfaid:conformance=\"A\"");
        let a = validate_pdf_bytes(
            &fixture(Some(a_xmp.as_bytes()), true),
            Some(ValidationProfile::PdfA1a),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert_no_rule(&a, "PDFA1A-ID-CONFORMANCE-001");
        assert_eq!(
            a.checks.total,
            ValidationProfile::PdfA1a.implemented_check_count()
        );
    }

    #[test]
    fn rejects_lowercase_pdfa_conformance() {
        let xmp = String::from_utf8(VALID_XMP.to_vec())
            .expect("fixture is UTF-8")
            .replace("pdfaid:conformance=\"B\"", "pdfaid:conformance=\"b\"");
        let report = validate_pdf_bytes(
            &fixture(Some(xmp.as_bytes()), true),
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert_rule(&report, "PDFA1B-ID-CONFORMANCE-001");
    }

    #[test]
    fn malformed_pdf_returns_a_parser_error() {
        let error = validate_pdf_bytes(
            include_bytes!("../tests/fixtures/malformed.pdf"),
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect_err("malformed PDF");
        assert!(matches!(error, ValidationError::Pdf(PdfError::Parse(_))));
    }

    #[test]
    fn reports_real_encrypted_input_as_conformance_failure() {
        let report = validate_pdf_bytes(
            include_bytes!("../tests/fixtures/encrypted.pdf"),
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
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

        let report = validate_pdf_bytes(
            &bytes,
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");

        assert_rule(&report, "PDFA1B-ENCRYPTION-001");
        let document = report.document.as_ref().expect("encrypted PDF is parsed");
        assert!(document.encrypted);
        assert!(!document.encrypted_content_unavailable);
        assert!(document.catalog_present);
        assert!(document.xmp.is_some());
        assert_eq!(
            report.checks.total,
            ValidationProfile::PdfA1b.implemented_check_count()
        );
    }

    #[test]
    fn encryption_takes_precedence_over_object_safety_limit() {
        let limits = SafetyLimits {
            max_object_count: 0,
            ..SafetyLimits::default()
        };
        let report = validate_pdf_bytes(
            include_bytes!("../tests/fixtures/encrypted.pdf"),
            Some(ValidationProfile::PdfA1b),
            &limits,
        )
        .expect("explicit profile validation");
        assert_rule(&report, "PDFA1B-ENCRYPTION-001");
        assert!(!report.has_operational_failure());
        assert_eq!(report.exit_code(), 2);
    }

    #[test]
    fn ordinary_object_safety_limit_returns_an_error() {
        let limits = SafetyLimits {
            max_object_count: 0,
            ..SafetyLimits::default()
        };
        let error = validate_pdf_bytes(
            include_bytes!("../tests/fixtures/structural.pdf"),
            Some(ValidationProfile::PdfA1b),
            &limits,
        )
        .expect_err("object limit");
        assert!(matches!(
            error,
            ValidationError::Pdf(PdfError::TooManyObjects { .. })
        ));
    }

    #[test]
    fn missing_input_returns_an_io_error() {
        let path = Path::new("tests/fixtures/definitely-not-present.pdf");
        let error = validate_pdf(
            path,
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect_err("missing input");
        assert!(matches!(error, ValidationError::InputIo(_)));
    }

    #[test]
    fn input_size_limit_returns_an_error() {
        let limits = SafetyLimits {
            max_input_size: 1,
            ..SafetyLimits::default()
        };
        let error = validate_pdf_bytes(
            include_bytes!("../tests/fixtures/structural.pdf"),
            Some(ValidationProfile::PdfA1b),
            &limits,
        )
        .expect_err("input size limit");
        assert!(matches!(
            error,
            ValidationError::Pdf(PdfError::InputTooLarge { .. })
        ));
    }

    #[test]
    fn decoded_stream_size_limit_returns_an_error() {
        let limits = SafetyLimits {
            max_decoded_stream_size: 16,
            ..SafetyLimits::default()
        };
        let error = validate_pdf_bytes(
            &fixture(Some(VALID_XMP), true),
            Some(ValidationProfile::PdfA1b),
            &limits,
        )
        .expect_err("decoded stream limit");
        assert!(matches!(
            error,
            ValidationError::Pdf(PdfError::XmpDecodeLimit(_))
        ));
    }

    #[test]
    fn reference_depth_limit_returns_an_error() {
        let limits = SafetyLimits {
            max_reference_depth: 0,
            ..SafetyLimits::default()
        };
        let error = validate_pdf_bytes(
            &fixture(Some(VALID_XMP), true),
            Some(ValidationProfile::PdfA1b),
            &limits,
        )
        .expect_err("reference depth limit");
        assert!(matches!(
            error,
            ValidationError::Pdf(PdfError::ReferenceDepth(0))
        ));
    }

    #[test]
    fn direct_root_dictionary_fails_catalog_check() {
        let report = validate_pdf_bytes(
            &fixture_with_root(Some(VALID_XMP), true, false),
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert_rule(&report, "PDFA1B-CATALOG-001");
    }

    #[test]
    fn static_structural_fixture_parses() {
        let report = validate_pdf_bytes(
            include_bytes!("../tests/fixtures/structural.pdf"),
            Some(ValidationProfile::PdfA1b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert!(
            report.document.is_some(),
            "fixture should parse: {:#?}",
            report.failures
        );
        assert!(!report.is_compliant, "fixture intentionally has no XMP");
    }

    #[test]
    fn same_rule_aggregators_discard_duplicate_findings() {
        let repeated = RuleFailure {
            object_id: Some(PdfObjectId {
                object_number: 10,
                generation: 0,
            }),
            description: "the same problem".to_owned(),
        };
        let distinct = RuleFailure {
            object_id: Some(PdfObjectId {
                object_number: 11,
                generation: 0,
            }),
            description: "the same problem".to_owned(),
        };
        let other = RuleFailure {
            object_id: Some(PdfObjectId {
                object_number: 12,
                generation: 0,
            }),
            description: "another problem".to_owned(),
        };
        let invalid = vec![repeated.clone(), repeated, distinct, other];

        let mut failures = Vec::new();
        aggregate_failures_with_location(&invalid, "TEST-001", None, &mut failures);
        assert_eq!(failures[0].message, "the same problem");

        let mut failures = Vec::new();
        aggregate_failures(&invalid, "TEST-001", &mut failures);
        assert_eq!(failures[0].message, "the same problem");
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
