use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes};

pub mod common;

const RULE: &str = "PDFUA1-TABLE-CELL-INTERSECTION-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:15";

#[test]
fn pdfua1_rule_7_2_15_rejects_intersecting_table_cells() {
    let allowed = validate_bytes(
        include_bytes!("fixtures/pdfua1-rule-7-2-15-allowed.pdf"),
        Some(ValidationProfile::PdfUa1),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");
    assert!(allowed.checks_passed, "{allowed}");
    assert!(allowed.failures.is_empty());

    let invalid = validate_bytes(
        include_bytes!("fixtures/pdfua1-rule-7-2-15-invalid.pdf"),
        Some(ValidationProfile::PdfUa1),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");
    assert!(!invalid.checks_passed, "{invalid}");
    assert_eq!(invalid.failures.len(), 1);
    assert_eq!(
        invalid
            .failures
            .iter()
            .filter(|failure| failure.rule_id == RULE)
            .count(),
        1
    );
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.2-15 fixtures"]
fn regenerate_pdfua1_rule_7_2_15_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-2-15-allowed.pdf", "allowed"),
        ("pdfua1-rule-7-2-15-invalid.pdf", "invalid"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_2_15_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.2-15 fixture");
    }
}

#[test]
fn pdfua1_rule_7_2_15_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-2-15-allowed.pdf", false),
        ("pdfua1-rule-7-2-15-invalid.pdf", true),
    ] {
        let report = runner.compare_file(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(fixture),
            &SafetyLimits::default(),
        );
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
