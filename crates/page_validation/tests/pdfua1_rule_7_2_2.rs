use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes};

pub mod common;

const RULE: &str = "PDFUA1-OUTLINE-LANGUAGE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:2";

#[test]
fn pdfua1_rule_7_2_2_fixtures_require_catalog_language_for_outline_entries() {
    let language_present = validate_bytes(
        include_bytes!("fixtures/pdfua1-rule-7-2-2-language-present.pdf"),
        Some(ValidationProfile::PdfUa1),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");
    assert!(
        !language_present
            .failures
            .iter()
            .any(|failure| failure.rule_id == RULE)
    );

    let language_missing = validate_bytes(
        include_bytes!("fixtures/pdfua1-rule-7-2-2-language-missing.pdf"),
        Some(ValidationProfile::PdfUa1),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");
    assert!(!language_missing.checks_passed, "{language_missing}");
    assert!(
        language_missing
            .failures
            .iter()
            .any(|failure| failure.rule_id == RULE)
    );
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.2-2 fixtures"]
fn regenerate_pdfua1_rule_7_2_2_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-2-2-language-present.pdf", "language_present"),
        ("pdfua1-rule-7-2-2-language-missing.pdf", "language_missing"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_2_2_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.2-2 fixture");
    }
}

#[test]
fn pdfua1_rule_7_2_2_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-2-2-language-present.pdf", false),
        ("pdfua1-rule-7-2-2-language-missing.pdf", true),
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
