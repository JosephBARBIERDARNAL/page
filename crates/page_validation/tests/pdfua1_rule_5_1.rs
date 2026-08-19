use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-ID-SCHEMA-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:5:1";

#[test]
fn pdfua1_rule_5_1_fixtures_enforce_identification_schema_presence() {
    let present = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-5-1-present.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(present.checks_passed, "{present}");
    assert_eq!(present.checks.total, 32);
    assert_eq!(present.checks.passed, 32);
    assert!(present.failures.is_empty());

    let missing = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-5-1-missing.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!missing.checks_passed, "{missing}");
    assert_eq!(missing.checks.total, 32);
    assert_eq!(missing.checks.failed, 2);
    assert_eq!(missing.failures.len(), 2);
    assert!(
        missing
            .failures
            .iter()
            .any(|failure| failure.rule_id == RULE)
    );
    assert!(
        missing
            .failures
            .iter()
            .any(|failure| failure.rule_id == "PDFUA1-ID-PART-001")
    );
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 5-1 fixtures"]
fn regenerate_pdfua1_rule_5_1_fixtures() {
    fs::write(
        "tests/fixtures/pdfua1-rule-5-1-present.pdf",
        common::pdfua1_rule_5_1_fixture("identification_present"),
    )
    .expect("write PDF/UA-1 pass fixture");
    fs::write(
        "tests/fixtures/pdfua1-rule-5-1-missing.pdf",
        common::pdfua1_rule_5_1_fixture("identification_missing"),
    )
    .expect("write PDF/UA-1 fail fixture");
}

#[test]
fn pdfua1_rule_5_1_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (fixture, should_fail) in [
        ("pdfua1-rule-5-1-present.pdf", false),
        ("pdfua1-rule-5-1-missing.pdf", true),
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
