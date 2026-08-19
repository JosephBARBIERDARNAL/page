use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-METADATA-STRUCTURE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.1:8";

#[test]
fn pdfua1_rule_7_1_8_fixtures_require_catalog_metadata_stream_structure() {
    let valid = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-1-8-valid.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(valid.checks_passed, "{valid}");
    assert_eq!(valid.checks.total, 20);
    assert_eq!(valid.checks.passed, 20);
    assert!(valid.failures.is_empty());

    for (fixture, expected_failed) in [
        ("pdfua1-rule-7-1-8-missing.pdf", 4),
        ("pdfua1-rule-7-1-8-wrong-type.pdf", 1),
        ("pdfua1-rule-7-1-8-wrong-subtype.pdf", 1),
    ] {
        let report = validate_bytes_with_profile(
            match fixture {
                "pdfua1-rule-7-1-8-missing.pdf" => {
                    include_bytes!("fixtures/pdfua1-rule-7-1-8-missing.pdf")
                }
                "pdfua1-rule-7-1-8-wrong-type.pdf" => {
                    include_bytes!("fixtures/pdfua1-rule-7-1-8-wrong-type.pdf")
                }
                "pdfua1-rule-7-1-8-wrong-subtype.pdf" => {
                    include_bytes!("fixtures/pdfua1-rule-7-1-8-wrong-subtype.pdf")
                }
                _ => unreachable!(),
            },
            ValidationProfile::PdfUa1,
            &SafetyLimits::default(),
        );
        assert!(!report.checks_passed, "{fixture}: {report}");
        assert_eq!(report.checks.total, 20);
        assert_eq!(report.checks.failed, expected_failed);
        assert_eq!(report.failures.len(), expected_failed);
        assert!(
            report
                .failures
                .iter()
                .any(|failure| failure.rule_id == RULE)
        );
    }
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.1-8 fixtures"]
fn regenerate_pdfua1_rule_7_1_8_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-1-8-valid.pdf", "valid"),
        ("pdfua1-rule-7-1-8-missing.pdf", "missing"),
        ("pdfua1-rule-7-1-8-wrong-type.pdf", "wrong_type"),
        ("pdfua1-rule-7-1-8-wrong-subtype.pdf", "wrong_subtype"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_1_8_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.1-8 fixture");
    }
}

#[test]
fn pdfua1_rule_7_1_8_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-1-8-valid.pdf", false),
        ("pdfua1-rule-7-1-8-missing.pdf", true),
        ("pdfua1-rule-7-1-8-wrong-type.pdf", true),
        ("pdfua1-rule-7-1-8-wrong-subtype.pdf", true),
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
