use std::fmt;
use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::limits::SafetyLimits;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidationRule {
    pub id: &'static str,
    pub description: &'static str,
}

const RULES: [ValidationRule; 7] = [
    ValidationRule {
        id: "PDF-PARSE-001",
        description: "The input parses as a PDF",
    },
    ValidationRule {
        id: "PDFA1B-ENCRYPTION-001",
        description: "The document is not encrypted",
    },
    ValidationRule {
        id: "PDFA1B-CATALOG-001",
        description: "The document catalog exists",
    },
    ValidationRule {
        id: "PDFA1B-XMP-001",
        description: "XMP metadata exists and parses",
    },
    ValidationRule {
        id: "PDFA1B-ID-PART-001",
        description: "XMP declares PDF/A part 1",
    },
    ValidationRule {
        id: "PDFA1B-ID-CONFORMANCE-001",
        description: "XMP declares PDF/A-1 conformance level A or B",
    },
    ValidationRule {
        id: "PDFA1B-OUTPUTINTENT-001",
        description: "At least one output intent exists",
    },
];

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
    let document = match PdfDocument::from_bytes(bytes, limits) {
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
    validate_document(document, profile)
}

fn validate_document(document: PdfDocument, profile: ValidationProfile) -> ValidationReport {
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

    if let Some(error) = &document.xmp_parse_error {
        failures.push(failure(
            "PDFA1B-XMP-001",
            format!("XMP metadata cannot be parsed: {error}"),
            document.xmp_object,
            FailureCategory::Metadata,
        ));
    } else if document.xmp.is_none() {
        failures.push(failure(
            "PDFA1B-XMP-001",
            "document catalog has no XMP metadata stream",
            None,
            FailureCategory::Metadata,
        ));
    }

    match document
        .xmp
        .as_ref()
        .and_then(|xmp| xmp.pdfa_part.as_deref())
    {
        Some("1") => {}
        Some(value) => failures.push(failure(
            "PDFA1B-ID-PART-001",
            format!("XMP declares PDF/A part {value}, expected 1"),
            document.xmp_object,
            FailureCategory::Metadata,
        )),
        None => failures.push(failure(
            "PDFA1B-ID-PART-001",
            "XMP has no pdfaid:part declaration",
            document.xmp_object,
            FailureCategory::Metadata,
        )),
    }

    match document
        .xmp
        .as_ref()
        .and_then(|xmp| xmp.pdfa_conformance.as_deref())
    {
        Some("A" | "B") => {}
        Some(value) => failures.push(failure(
            "PDFA1B-ID-CONFORMANCE-001",
            format!("XMP declares PDF/A conformance {value}, expected A or B"),
            document.xmp_object,
            FailureCategory::Metadata,
        )),
        None => failures.push(failure(
            "PDFA1B-ID-CONFORMANCE-001",
            "XMP has no pdfaid:conformance declaration",
            document.xmp_object,
            FailureCategory::Metadata,
        )),
    }

    if document.output_intents.is_empty() {
        failures.push(failure(
            "PDFA1B-OUTPUTINTENT-001",
            "document catalog has no output intent",
            document.catalog_reference,
            FailureCategory::Conformance,
        ));
    }

    finish_report(document, profile, failures, RULES.len())
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
    use lopdf::{Document, Object, Stream, dictionary};

    use super::*;

    const VALID_XMP: &[u8] = br#"<?xpacket begin=""?>
      <x:xmpmeta xmlns:x="adobe:ns:meta/">
        <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
          <rdf:Description xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/"
            pdfaid:part="1" pdfaid:conformance="B"/>
        </rdf:RDF>
      </x:xmpmeta>
      <?xpacket end="w"?>"#;

    #[test]
    fn accepts_all_implemented_checks() {
        let bytes = fixture(Some(VALID_XMP), true);
        let report = validate_bytes(&bytes, ValidationProfile::PdfA1b, &SafetyLimits::default());
        assert!(report.implemented_checks_passed, "{:#?}", report.failures);
        assert_eq!(report.checks.passed, 7);
    }

    #[test]
    fn reports_missing_xmp() {
        let report = validate_bytes(
            &fixture(None, true),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_rule(&report, "PDFA1B-XMP-001");
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
    fn reports_missing_output_intent() {
        let report = validate_bytes(
            &fixture(Some(VALID_XMP), false),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert_rule(&report, "PDFA1B-OUTPUTINTENT-001");
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
}
