use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-TAGGED-DOCUMENT-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:6.2:1";

#[test]
fn pdfua1_rule_6_2_fixtures_require_mark_info_marked_true() {
    let marked = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-6-2-marked.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(marked.checks_passed, "{marked}");
    assert_eq!(marked.checks.total, 9);
    assert_eq!(marked.checks.passed, 9);
    assert!(marked.failures.is_empty());

    let unmarked = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-6-2-unmarked.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!unmarked.checks_passed, "{unmarked}");
    assert_eq!(unmarked.checks.total, 9);
    assert_eq!(unmarked.checks.failed, 1);
    assert_eq!(unmarked.failures.len(), 1);
    assert_eq!(unmarked.failures[0].rule_id, RULE);
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 6.2-1 fixtures"]
fn regenerate_pdfua1_rule_6_2_fixtures() {
    fs::write(
        "tests/fixtures/pdfua1-rule-6-2-marked.pdf",
        common::pdfua1_rule_6_2_fixture("marked_true"),
    )
    .expect("write PDF/UA-1 rule 6.2-1 pass fixture");
    fs::write(
        "tests/fixtures/pdfua1-rule-6-2-unmarked.pdf",
        common::pdfua1_rule_6_2_fixture("marked_false"),
    )
    .expect("write PDF/UA-1 rule 6.2-1 fail fixture");
}

#[test]
fn pdfua1_rule_6_2_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (fixture, should_fail) in [
        ("pdfua1-rule-6-2-marked.pdf", false),
        ("pdfua1-rule-6-2-unmarked.pdf", true),
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
