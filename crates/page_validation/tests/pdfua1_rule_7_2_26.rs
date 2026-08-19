use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-TOCI-PARENT-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:26";

#[test]
fn pdfua1_rule_7_2_26_requires_toci_to_be_contained_in_toc() {
    let contained = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-2-26-contained.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(contained.checks_passed, "{contained}");
    assert_eq!(contained.checks.total, 30);
    assert_eq!(contained.checks.passed, 30);
    assert!(contained.failures.is_empty());

    let not_contained = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-2-26-not-contained.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!not_contained.checks_passed, "{not_contained}");
    assert_eq!(not_contained.checks.total, 30);
    assert_eq!(not_contained.checks.failed, 1);
    assert_eq!(not_contained.failures.len(), 1);
    assert_eq!(not_contained.failures[0].rule_id, RULE);
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.2-26 fixtures"]
fn regenerate_pdfua1_rule_7_2_26_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-2-26-contained.pdf", "contained"),
        ("pdfua1-rule-7-2-26-not-contained.pdf", "not_contained"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_2_26_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.2-26 fixture");
    }
}

#[test]
fn pdfua1_rule_7_2_26_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-2-26-contained.pdf", false),
        ("pdfua1-rule-7-2-26-not-contained.pdf", true),
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
