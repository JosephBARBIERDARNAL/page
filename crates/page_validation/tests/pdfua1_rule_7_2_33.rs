use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-METADATA-LANGUAGE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:33";

#[test]
fn pdfua1_rule_7_2_33_requires_an_x_default_language_alternative_or_catalog_language() {
    for case in ["x_default", "catalog_language"] {
        let report = validate_bytes_with_profile(
            &common::pdfua1_rule_7_2_33_fixture(case),
            ValidationProfile::PdfUa1,
            &SafetyLimits::default(),
        );
        assert_eq!(report.checks.total, 33);
        assert!(
            !report
                .failures
                .iter()
                .any(|failure| failure.rule_id == RULE)
        );
    }

    for case in ["multiple_items", "missing_x_default"] {
        let report = validate_bytes_with_profile(
            &common::pdfua1_rule_7_2_33_fixture(case),
            ValidationProfile::PdfUa1,
            &SafetyLimits::default(),
        );
        assert!(!report.checks_passed, "{case}: {report}");
        assert_eq!(report.checks.total, 33);
        assert!(
            report
                .failures
                .iter()
                .any(|failure| failure.rule_id == RULE)
        );
    }
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.2-33 fixtures"]
fn regenerate_pdfua1_rule_7_2_33_fixtures() {
    for case in [
        "x_default",
        "catalog_language",
        "multiple_items",
        "missing_x_default",
    ] {
        fs::write(
            Path::new("tests/fixtures").join(format!("pdfua1-rule-7-2-33-{case}.pdf")),
            common::pdfua1_rule_7_2_33_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.2-33 fixture");
    }
}

#[test]
fn pdfua1_rule_7_2_33_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    // veraPDF 1.30.2 reports 7.2-33 for the canonical one-item x-default
    // fixture, although its published predicate and source implementation
    // define that value as passing. It does not emit the rule for the two
    // malformed language alternatives, so the discrepancy is pinned here.
    for (case, reference_should_fail_rule) in [
        ("x_default", true),
        ("catalog_language", false),
        ("multiple_items", false),
        ("missing_x_default", false),
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(format!("pdfua1-rule-7-2-33-{case}.pdf"));
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
            reference_should_fail_rule,
            "{case}: {report}"
        );
    }
}
