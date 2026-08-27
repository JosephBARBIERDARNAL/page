use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes};

pub mod common;

const RULE: &str = "PDFUA1-TABLE-HEADERS-UNDEFINED-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.5:2";

#[test]
fn pdfua1_rule_7_5_2_requires_scope_for_undefined_headers() {
    let valid = validate_bytes(
        include_bytes!("fixtures/pdfua1-rule-7-5-2-scope-present.pdf"),
        Some(ValidationProfile::PdfUa1),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");
    assert!(valid.checks_passed, "{valid}");
    assert!(valid.failures.is_empty());

    let invalid = validate_bytes(
        include_bytes!("fixtures/pdfua1-rule-7-5-2-scope-missing.pdf"),
        Some(ValidationProfile::PdfUa1),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");
    assert!(!invalid.checks_passed, "{invalid}");
    assert_eq!(invalid.checks.failed, 1, "{invalid}");
    assert_eq!(invalid.failures.len(), 1, "{invalid}");
    assert_eq!(invalid.failures[0].rule_id, RULE, "{invalid}");
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.5-2 fixtures"]
fn regenerate_pdfua1_rule_7_5_2_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-5-2-scope-present.pdf", "scope_present"),
        ("pdfua1-rule-7-5-2-scope-missing.pdf", "scope_missing"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_5_2_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.5-2 fixture");
    }
}

#[test]
fn pdfua1_rule_7_5_2_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-5-2-scope-present.pdf", false),
        ("pdfua1-rule-7-5-2-scope-missing.pdf", true),
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
