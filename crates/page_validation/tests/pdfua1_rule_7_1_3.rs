use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-CONTENT-TAGGING-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.1:3";

#[test]
fn pdfua1_rule_7_1_3_requires_artifact_or_tagged_painted_content() {
    for (fixture, description) in [
        (
            include_bytes!("fixtures/pdfua1-rule-7-1-3-artifact.pdf").as_slice(),
            "artifact",
        ),
        (
            include_bytes!("fixtures/pdfua1-rule-7-1-3-tagged.pdf").as_slice(),
            "tagged",
        ),
    ] {
        let report = validate_bytes_with_profile(
            fixture,
            ValidationProfile::PdfUa1,
            &SafetyLimits::default(),
        );
        assert!(report.checks_passed, "{description}: {report}");
        assert_eq!(report.checks.total, 15);
        assert_eq!(report.checks.passed, 15);
    }

    let report = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-1-3-untagged.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!report.checks_passed, "{report}");
    assert_eq!(report.checks.total, 15);
    assert_eq!(report.checks.failed, 1);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].rule_id, RULE);
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.1-3 fixtures"]
fn regenerate_pdfua1_rule_7_1_3_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-1-3-artifact.pdf", "artifact"),
        ("pdfua1-rule-7-1-3-tagged.pdf", "tagged"),
        ("pdfua1-rule-7-1-3-untagged.pdf", "untagged"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_1_3_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.1-3 fixture");
    }
}

#[test]
fn pdfua1_rule_7_1_3_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-1-3-artifact.pdf", false),
        ("pdfua1-rule-7-1-3-tagged.pdf", false),
        ("pdfua1-rule-7-1-3-untagged.pdf", true),
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
