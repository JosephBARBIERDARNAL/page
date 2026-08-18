use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-SUSPECTS-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.1:4";

#[test]
fn pdfua1_rule_7_1_4_fixtures_reject_suspects_true() {
    let suspects_false = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-1-4-false.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(suspects_false.checks_passed, "{suspects_false}");
    assert_eq!(suspects_false.checks.total, 19);
    assert_eq!(suspects_false.checks.passed, 19);
    assert!(suspects_false.failures.is_empty());

    let suspects_true = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-1-4-true.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!suspects_true.checks_passed, "{suspects_true}");
    assert_eq!(suspects_true.checks.total, 19);
    assert_eq!(suspects_true.checks.failed, 1);
    assert_eq!(suspects_true.failures.len(), 1);
    assert_eq!(suspects_true.failures[0].rule_id, RULE);
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.1-4 fixtures"]
fn regenerate_pdfua1_rule_7_1_4_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-1-4-false.pdf", "false"),
        ("pdfua1-rule-7-1-4-true.pdf", "true"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_1_4_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.1-4 fixture");
    }
}

#[test]
fn pdfua1_rule_7_1_4_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-1-4-false.pdf", false),
        ("pdfua1-rule-7-1-4-true.pdf", true),
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
