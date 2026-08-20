use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-VIEWER-PREFERENCES-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.1:10";

#[test]
fn pdfua1_rule_7_1_10_fixtures_require_display_doc_title_true() {
    let present = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-1-10-present.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(present.checks_passed, "{present}");
    assert_eq!(present.checks.total, 33);
    assert_eq!(present.checks.passed, 33);
    assert!(present.failures.is_empty());

    for fixture in [
        "pdfua1-rule-7-1-10-false.pdf",
        "pdfua1-rule-7-1-10-missing.pdf",
    ] {
        let report = validate_bytes_with_profile(
            match fixture {
                "pdfua1-rule-7-1-10-false.pdf" => {
                    include_bytes!("fixtures/pdfua1-rule-7-1-10-false.pdf")
                }
                "pdfua1-rule-7-1-10-missing.pdf" => {
                    include_bytes!("fixtures/pdfua1-rule-7-1-10-missing.pdf")
                }
                _ => unreachable!(),
            },
            ValidationProfile::PdfUa1,
            &SafetyLimits::default(),
        );
        assert!(!report.checks_passed, "{fixture}: {report}");
        assert_eq!(report.checks.total, 33);
        assert_eq!(report.checks.failed, 1);
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].rule_id, RULE);
    }
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.1-10 fixtures"]
fn regenerate_pdfua1_rule_7_1_10_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-1-10-present.pdf", "present"),
        ("pdfua1-rule-7-1-10-false.pdf", "false"),
        ("pdfua1-rule-7-1-10-missing.pdf", "missing"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_1_10_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.1-10 fixture");
    }
}

#[test]
fn pdfua1_rule_7_1_10_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-1-10-present.pdf", false),
        ("pdfua1-rule-7-1-10-false.pdf", true),
        ("pdfua1-rule-7-1-10-missing.pdf", true),
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
