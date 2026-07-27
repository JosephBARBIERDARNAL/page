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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidationRule {
    pub id: &'static str,
    pub description: &'static str,
}

const RULES: [ValidationRule; 18] = [
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
        description: "The metadata stream is parseable XML",
    },
    ValidationRule {
        id: "PDFA1B-METADATA-STRUCTURE-001",
        description: "Catalog Metadata resolves to a /Metadata /XML stream",
    },
    ValidationRule {
        id: "PDFA1B-METADATA-FILTER-001",
        description: "The catalog metadata stream has no Filter key",
    },
    ValidationRule {
        id: "PDFA1B-ID-SCHEMA-001",
        description: "XMP contains the PDF/A Identification schema",
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
        id: "PDFA1B-INFO-CREATIONDATE-001",
        description: "Info CreationDate is equivalent to xmp:CreateDate",
    },
    ValidationRule {
        id: "PDFA1B-INFO-TITLE-001",
        description: "Info Title equals dc:title['x-default']",
    },
    ValidationRule {
        id: "PDFA1B-INFO-AUTHOR-001",
        description: "Info Author equals the sole dc:creator entry",
    },
    ValidationRule {
        id: "PDFA1B-INFO-SUBJECT-001",
        description: "Info Subject equals dc:description['x-default']",
    },
    ValidationRule {
        id: "PDFA1B-INFO-KEYWORDS-001",
        description: "Info Keywords equals pdf:Keywords",
    },
    ValidationRule {
        id: "PDFA1B-INFO-CREATOR-001",
        description: "Info Creator equals xmp:CreatorTool",
    },
    ValidationRule {
        id: "PDFA1B-INFO-PRODUCER-001",
        description: "Info Producer equals pdf:Producer",
    },
    ValidationRule {
        id: "PDFA1B-INFO-MODDATE-001",
        description: "Info ModDate is equivalent to xmp:ModifyDate",
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

    let metadata = &document.catalog_metadata;
    if !(metadata.present
        && metadata.is_stream
        && metadata.type_is_metadata
        && metadata.subtype_is_xml)
    {
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
    if !xmp.is_some_and(|xmp| xmp.pdfa_identification_present) {
        failures.push(failure(
            "PDFA1B-ID-SCHEMA-001",
            "XMP does not contain the PDF/A Identification schema",
            document.xmp_object,
            FailureCategory::Metadata,
        ));
    }

    match xmp.map(|xmp| xmp.pdfa_parts.as_slice()) {
        Some([value]) if value == "1" => {}
        Some(values) => failures.push(failure(
            "PDFA1B-ID-PART-001",
            format!("XMP declares PDF/A part values {values:?}, expected exactly one value 1"),
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

    match xmp.map(|xmp| xmp.pdfa_conformances.as_slice()) {
        Some([value]) if matches!(value.as_str(), "A" | "B") => {}
        Some(values) => failures.push(failure(
            "PDFA1B-ID-CONFORMANCE-001",
            format!(
                "XMP declares PDF/A conformance values {values:?}, expected exactly one A or B"
            ),
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

    validate_info_consistency(&document, &mut failures);

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

fn validate_info_consistency(document: &PdfDocument, failures: &mut Vec<ValidationFailure>) {
    let xmp = document.xmp.as_ref();
    let empty = &[];
    let object_id = document.info_object;
    compare_single(
        document,
        "Title",
        xmp.map_or(empty, |xmp| &xmp.title_x_default),
        "PDFA1B-INFO-TITLE-001",
        "dc:title['x-default']",
        object_id,
        failures,
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
    compare_single(
        document,
        "Subject",
        xmp.map_or(empty, |xmp| &xmp.description_x_default),
        "PDFA1B-INFO-SUBJECT-001",
        "dc:description['x-default']",
        object_id,
        failures,
    );
    compare_single(
        document,
        "Keywords",
        xmp.map_or(empty, |xmp| &xmp.keywords),
        "PDFA1B-INFO-KEYWORDS-001",
        "pdf:Keywords",
        object_id,
        failures,
    );
    compare_single(
        document,
        "Creator",
        xmp.map_or(empty, |xmp| &xmp.creator_tools),
        "PDFA1B-INFO-CREATOR-001",
        "xmp:CreatorTool",
        object_id,
        failures,
    );
    compare_single(
        document,
        "Producer",
        xmp.map_or(empty, |xmp| &xmp.producers),
        "PDFA1B-INFO-PRODUCER-001",
        "pdf:Producer",
        object_id,
        failures,
    );
    if let Some(xmp) = xmp {
        compare_date(
            document,
            "CreationDate",
            &xmp.create_dates,
            "PDFA1B-INFO-CREATIONDATE-001",
            "xmp:CreateDate",
            object_id,
            failures,
        );
        compare_date(
            document,
            "ModDate",
            &xmp.modify_dates,
            "PDFA1B-INFO-MODDATE-001",
            "xmp:ModifyDate",
            object_id,
            failures,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn compare_single(
    document: &PdfDocument,
    info_key: &str,
    xmp_values: &[String],
    rule_id: &'static str,
    xmp_name: &str,
    object_id: Option<crate::PdfObjectId>,
    failures: &mut Vec<ValidationFailure>,
) {
    if let Some(info_value) = document.info.values.get(info_key)
        && (xmp_values.len() != 1 || xmp_values.first() != Some(info_value))
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

#[allow(clippy::too_many_arguments)]
fn compare_date(
    document: &PdfDocument,
    info_key: &str,
    xmp_values: &[String],
    rule_id: &'static str,
    xmp_name: &str,
    object_id: Option<crate::PdfObjectId>,
    failures: &mut Vec<ValidationFailure>,
) {
    if let Some(info_value) = document.info.values.get(info_key)
        && (xmp_values.len() != 1
            || !xmp_values
                .first()
                .is_some_and(|xmp| dates_equivalent(info_value, xmp)))
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
        assert_eq!(report.checks.passed, RULES.len());
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
