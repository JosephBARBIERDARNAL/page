use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_pdf_bytes};

pub mod common;

const RULE: &str = "PDFUA1-TABLE-ROW-COLUMNSPAN-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:42";
const REFERENCE_COMPANION_RULE: &str = "ISO 14289-1:2014:7.2:43";

#[test]
fn pdfua1_rule_7_2_42_requires_equal_row_column_spans() {
    let allowed = validate_pdf_bytes(
        include_bytes!("fixtures/pdfua1-rule-7-2-42-allowed.pdf"),
        Some(ValidationProfile::PdfUa1),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");
    assert!(allowed.is_compliant, "{allowed}");
    assert!(allowed.failures.is_empty());

    let invalid = validate_pdf_bytes(
        include_bytes!("fixtures/pdfua1-rule-7-2-42-invalid.pdf"),
        Some(ValidationProfile::PdfUa1),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");
    assert!(!invalid.is_compliant, "{invalid}");
    assert_eq!(invalid.checks.failed, 1);
    assert_eq!(invalid.failures.len(), 1);
    assert_eq!(invalid.failures[0].rule_id, RULE);
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.2-42 fixtures"]
fn regenerate_pdfua1_rule_7_2_42_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-2-42-allowed.pdf", "allowed"),
        ("pdfua1-rule-7-2-42-invalid.pdf", "invalid"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_2_42_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.2-42 fixture");
    }
}

#[test]
fn pdfua1_rule_7_2_42_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");

    let allowed = runner.compare_file(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/pdfua1-rule-7-2-42-allowed.pdf"),
        &SafetyLimits::default(),
    );
    let allowed_failed = allowed
        .reference_result
        .as_ref()
        .expect("veraPDF result")
        .failed_rule_ids
        .iter()
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>();
    assert!(!allowed_failed.contains(REFERENCE_RULE), "{allowed}");
    assert!(
        !allowed_failed.contains(REFERENCE_COMPANION_RULE),
        "{allowed}"
    );

    let invalid = runner.compare_file(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/pdfua1-rule-7-2-42-invalid.pdf"),
        &SafetyLimits::default(),
    );
    let invalid_failed = invalid
        .reference_result
        .as_ref()
        .expect("veraPDF result")
        .failed_rule_ids
        .iter()
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>();
    assert!(invalid_failed.contains(REFERENCE_RULE), "{invalid}");
    assert!(
        !invalid_failed.contains(REFERENCE_COMPANION_RULE),
        "{invalid}"
    );
}
