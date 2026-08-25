use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-TABLE-THEAD-TBODY-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:14";

#[test]
fn pdfua1_rule_7_2_14_requires_tbody_when_table_has_thead() {
    let allowed = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-2-14-allowed.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(allowed.checks_passed, "{allowed}");
    assert!(allowed.failures.is_empty());

    let invalid = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-2-14-invalid.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!invalid.checks_passed, "{invalid}");
    assert_eq!(invalid.checks.failed, 1);
    assert_eq!(invalid.failures.len(), 1);
    assert_eq!(invalid.failures[0].rule_id, RULE);
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.2-14 fixtures"]
fn regenerate_pdfua1_rule_7_2_14_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-2-14-allowed.pdf", "allowed"),
        ("pdfua1-rule-7-2-14-invalid.pdf", "invalid"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_2_14_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.2-14 fixture");
    }
}

#[test]
fn pdfua1_rule_7_2_14_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-2-14-allowed.pdf", false),
        ("pdfua1-rule-7-2-14-invalid.pdf", true),
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
