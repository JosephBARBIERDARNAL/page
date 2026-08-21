use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-PAGE-TABS-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.18.3:1";

#[test]
fn pdfua1_rule_7_18_3_1_requires_tabs_s_on_pages_with_annotations() {
    let allowed = validate_bytes_with_profile(
        fixture_bytes("allowed"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(allowed.checks_passed, "{allowed}");
    assert!(allowed.failures.is_empty(), "{allowed}");
    assert_eq!(allowed.checks.total, 74, "{allowed}");

    for case in ["missing", "wrong"] {
        let invalid = validate_bytes_with_profile(
            fixture_bytes(case),
            ValidationProfile::PdfUa1,
            &SafetyLimits::default(),
        );
        assert!(!invalid.checks_passed, "{case}: {invalid}");
        assert_eq!(invalid.checks.failed, 1, "{case}: {invalid}");
        assert_eq!(invalid.failures.len(), 1, "{case}: {invalid}");
        assert_eq!(invalid.failures[0].rule_id, RULE, "{case}: {invalid}");
    }
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.18.3-1 fixtures"]
fn regenerate_pdfua1_rule_7_18_3_1_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-18-3-1-allowed.pdf", "allowed"),
        ("pdfua1-rule-7-18-3-1-missing.pdf", "missing"),
        ("pdfua1-rule-7-18-3-1-wrong.pdf", "wrong"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_18_3_1_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.18.3-1 fixture");
    }
}

#[test]
fn pdfua1_rule_7_18_3_1_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-18-3-1-allowed.pdf", false),
        ("pdfua1-rule-7-18-3-1-missing.pdf", true),
        ("pdfua1-rule-7-18-3-1-wrong.pdf", true),
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

fn fixture_bytes(case: &str) -> &'static [u8] {
    match case {
        "allowed" => include_bytes!("fixtures/pdfua1-rule-7-18-3-1-allowed.pdf"),
        "missing" => include_bytes!("fixtures/pdfua1-rule-7-18-3-1-missing.pdf"),
        "wrong" => include_bytes!("fixtures/pdfua1-rule-7-18-3-1-wrong.pdf"),
        _ => panic!("unknown fixture {case}"),
    }
}
