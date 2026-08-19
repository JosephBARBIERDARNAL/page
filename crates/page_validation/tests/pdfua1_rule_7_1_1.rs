use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-ARTIFACT-NESTED-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.1:1";

#[test]
fn pdfua1_rule_7_1_1_rejects_artifacts_inside_tagged_content() {
    let outside = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-1-1-outside-tagged-content.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(outside.checks_passed, "{outside}");
    assert_eq!(outside.checks.total, 30);
    assert_eq!(outside.checks.passed, 30);

    let inside = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-1-1-inside-tagged-content.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!inside.checks_passed, "{inside}");
    assert_eq!(inside.checks.total, 30);
    assert_eq!(inside.checks.failed, 1);
    assert_eq!(inside.failures.len(), 1);
    assert_eq!(inside.failures[0].rule_id, RULE);
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.1-1 fixtures"]
fn regenerate_pdfua1_rule_7_1_1_fixtures() {
    for (fixture, case) in [
        (
            "pdfua1-rule-7-1-1-outside-tagged-content.pdf",
            "artifact_outside_tagged_content",
        ),
        (
            "pdfua1-rule-7-1-1-inside-tagged-content.pdf",
            "artifact_inside_tagged_content",
        ),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_1_1_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.1-1 fixture");
    }
}

#[test]
fn pdfua1_rule_7_1_1_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-1-1-outside-tagged-content.pdf", false),
        ("pdfua1-rule-7-1-1-inside-tagged-content.pdf", true),
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
