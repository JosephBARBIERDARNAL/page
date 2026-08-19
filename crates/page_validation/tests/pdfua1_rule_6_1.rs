use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-HEADER-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:6.1:1";

#[test]
fn pdfua1_rule_6_1_fixtures_require_a_pdf_1_header_version_from_zero_to_seven() {
    let valid_header = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-6-1-valid-header.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(valid_header.checks_passed, "{valid_header}");
    assert_eq!(valid_header.checks.total, 26);
    assert_eq!(valid_header.checks.passed, 26);
    assert!(valid_header.failures.is_empty());

    let invalid_header = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-6-1-invalid-header.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!invalid_header.checks_passed, "{invalid_header}");
    assert_eq!(invalid_header.checks.total, 26);
    assert_eq!(invalid_header.checks.failed, 1);
    assert_eq!(invalid_header.failures.len(), 1);
    assert_eq!(invalid_header.failures[0].rule_id, RULE);
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 6.1-1 fixtures"]
fn regenerate_pdfua1_rule_6_1_fixtures() {
    fs::write(
        "tests/fixtures/pdfua1-rule-6-1-valid-header.pdf",
        common::pdfua1_rule_6_1_fixture("valid_header"),
    )
    .expect("write PDF/UA-1 rule 6.1-1 pass fixture");
    fs::write(
        "tests/fixtures/pdfua1-rule-6-1-invalid-header.pdf",
        common::pdfua1_rule_6_1_fixture("invalid_header"),
    )
    .expect("write PDF/UA-1 rule 6.1-1 fail fixture");
}

#[test]
fn pdfua1_rule_6_1_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (fixture, should_fail) in [
        ("pdfua1-rule-6-1-valid-header.pdf", false),
        ("pdfua1-rule-6-1-invalid-header.pdf", true),
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture);
        let report = runner.compare_file(&path, &SafetyLimits::default());
        let reference = report.reference_result.as_ref().expect("veraPDF result");
        let failed = reference
            .failed_rule_ids
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            failed.contains(REFERENCE_RULE),
            should_fail,
            "{fixture}: {report}"
        );
    }
}
