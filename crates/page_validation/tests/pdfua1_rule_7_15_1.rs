use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-DYNAMIC-XFA-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.15:1";

#[test]
fn pdfua1_rule_7_15_1_rejects_dynamic_xfa_forms_only() {
    for (case, bytes) in [
        (
            "no_xfa",
            include_bytes!("fixtures/pdfua1-rule-7-15-1-no-xfa.pdf").as_slice(),
        ),
        (
            "static_xfa",
            include_bytes!("fixtures/pdfua1-rule-7-15-1-static-xfa.pdf").as_slice(),
        ),
    ] {
        let report =
            validate_bytes_with_profile(bytes, ValidationProfile::PdfUa1, &SafetyLimits::default());
        assert!(report.checks_passed, "{case}: {report}");
        assert!(report.failures.is_empty(), "{case}: {report}");
    }

    let report = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-15-1-dynamic-xfa.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!report.checks_passed, "{report}");
    assert_eq!(report.checks.total, 77, "{report}");
    assert_eq!(report.checks.failed, 1, "{report}");
    assert_eq!(report.failures.len(), 1, "{report}");
    assert_eq!(report.failures[0].rule_id, RULE, "{report}");
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.15-1 fixtures"]
fn regenerate_pdfua1_rule_7_15_1_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-15-1-no-xfa.pdf", "no_xfa"),
        ("pdfua1-rule-7-15-1-static-xfa.pdf", "static_xfa"),
        ("pdfua1-rule-7-15-1-dynamic-xfa.pdf", "dynamic_xfa"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_15_1_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.15-1 fixture");
    }
}

#[test]
fn pdfua1_rule_7_15_1_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-15-1-no-xfa.pdf", false),
        ("pdfua1-rule-7-15-1-static-xfa.pdf", false),
        ("pdfua1-rule-7-15-1-dynamic-xfa.pdf", true),
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
