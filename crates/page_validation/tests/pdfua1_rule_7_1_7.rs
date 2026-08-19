use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-STRUCT-TREE-ROLE-MAP-STANDARD-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.1:7";

#[test]
fn pdfua1_rule_7_1_7_rejects_remapped_standard_types() {
    let standard_unmapped = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-1-7-standard-unmapped.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(standard_unmapped.checks_passed, "{standard_unmapped}");
    assert_eq!(standard_unmapped.checks.total, 27);
    assert_eq!(standard_unmapped.checks.passed, 27);
    assert!(standard_unmapped.failures.is_empty());

    let standard_remapped = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-1-7-standard-remapped.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!standard_remapped.checks_passed, "{standard_remapped}");
    assert_eq!(standard_remapped.checks.total, 27);
    assert_eq!(standard_remapped.checks.failed, 1);
    assert_eq!(standard_remapped.failures.len(), 1);
    assert_eq!(standard_remapped.failures[0].rule_id, RULE);
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.1-7 fixtures"]
fn regenerate_pdfua1_rule_7_1_7_fixtures() {
    for (fixture, case) in [
        (
            "pdfua1-rule-7-1-7-standard-unmapped.pdf",
            "standard_unmapped",
        ),
        (
            "pdfua1-rule-7-1-7-standard-remapped.pdf",
            "standard_remapped",
        ),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_1_7_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.1-7 fixture");
    }
}

#[test]
fn pdfua1_rule_7_1_7_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-1-7-standard-unmapped.pdf", false),
        ("pdfua1-rule-7-1-7-standard-remapped.pdf", true),
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
