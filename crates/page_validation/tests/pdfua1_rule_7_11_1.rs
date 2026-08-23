use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-FILE-SPEC-F-AND-UF-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.11:1";

#[test]
fn pdfua1_rule_7_11_1_requires_non_empty_f_and_uf_keys() {
    let valid = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-11-1-valid.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(valid.checks_passed, "{valid}");
    assert!(valid.failures.is_empty(), "{valid}");

    let empty_uf = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-11-1-empty-uf.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!empty_uf.checks_passed, "{empty_uf}");
    assert_eq!(empty_uf.checks.total, 80, "{empty_uf}");
    assert_eq!(empty_uf.checks.failed, 1, "{empty_uf}");
    assert_eq!(empty_uf.failures.len(), 1, "{empty_uf}");
    assert_eq!(empty_uf.failures[0].rule_id, RULE, "{empty_uf}");
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.11-1 fixtures"]
fn regenerate_pdfua1_rule_7_11_1_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-11-1-valid.pdf", "valid"),
        ("pdfua1-rule-7-11-1-empty-uf.pdf", "empty_uf"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_11_1_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.11-1 fixture");
    }
}

#[test]
fn pdfua1_rule_7_11_1_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-11-1-valid.pdf", false),
        ("pdfua1-rule-7-11-1-empty-uf.pdf", true),
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
