use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_pdf_bytes};

pub mod common;

const RULE: &str = "PDFUA1-HEADING-CHILD-COUNT-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.4.4:1";

#[test]
fn pdfua1_rule_7_4_4_1_allows_at_most_one_h_child_per_node() {
    let valid = validate_pdf_bytes(
        include_bytes!("fixtures/pdfua1-rule-7-4-4-1-single-h.pdf"),
        Some(ValidationProfile::PdfUa1),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");
    assert!(valid.is_compliant, "{valid}");
    assert!(valid.failures.is_empty());

    let invalid = validate_pdf_bytes(
        include_bytes!("fixtures/pdfua1-rule-7-4-4-1-multiple-h.pdf"),
        Some(ValidationProfile::PdfUa1),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");
    assert!(!invalid.is_compliant, "{invalid}");
    assert_eq!(invalid.checks.failed, 1, "{invalid}");
    assert_eq!(invalid.failures.len(), 1, "{invalid}");
    assert_eq!(invalid.failures[0].rule_id, RULE, "{invalid}");
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.4.4-1 fixtures"]
fn regenerate_pdfua1_rule_7_4_4_1_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-4-4-1-single-h.pdf", "single_h"),
        ("pdfua1-rule-7-4-4-1-multiple-h.pdf", "multiple_h"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_4_4_1_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.4.4-1 fixture");
    }
}

#[test]
fn pdfua1_rule_7_4_4_1_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-4-4-1-single-h.pdf", false),
        ("pdfua1-rule-7-4-4-1-multiple-h.pdf", true),
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture);
        let report = runner.compare_file(&path, &SafetyLimits::default());
        let failed = report
            .reference_result
            .as_ref()
            .expect("veraPDF result")
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
