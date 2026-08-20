use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-ANNOTATION-CONTENTS-LANGUAGE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:24";

#[test]
fn pdfua1_rule_7_2_24_requires_language_for_annotation_contents() {
    for (_fixture, bytes) in [
        (
            "pdfua1-rule-7-2-24-annotation-language-present.pdf",
            include_bytes!("fixtures/pdfua1-rule-7-2-24-annotation-language-present.pdf")
                as &[u8],
        ),
        (
            "pdfua1-rule-7-2-24-catalog-language-present.pdf",
            include_bytes!("fixtures/pdfua1-rule-7-2-24-catalog-language-present.pdf") as &[u8],
        ),
    ] {
        let report =
            validate_bytes_with_profile(bytes, ValidationProfile::PdfUa1, &SafetyLimits::default());
        assert_eq!(report.checks.total, 33);
        assert!(
            !report
                .failures
                .iter()
                .any(|failure| failure.rule_id == RULE)
        );
    }

    let language_missing = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-2-24-language-missing.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!language_missing.checks_passed, "{language_missing}");
    assert_eq!(language_missing.checks.total, 33);
    assert!(
        language_missing
            .failures
            .iter()
            .any(|failure| failure.rule_id == RULE)
    );
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.2-24 fixtures"]
fn regenerate_pdfua1_rule_7_2_24_fixtures() {
    for (fixture, case) in [
        (
            "pdfua1-rule-7-2-24-annotation-language-present.pdf",
            "annotation_language_present",
        ),
        (
            "pdfua1-rule-7-2-24-catalog-language-present.pdf",
            "catalog_language_present",
        ),
        (
            "pdfua1-rule-7-2-24-language-missing.pdf",
            "language_missing",
        ),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_2_24_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.2-24 fixture");
    }
}

#[test]
fn pdfua1_rule_7_2_24_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-2-24-annotation-language-present.pdf", false),
        ("pdfua1-rule-7-2-24-catalog-language-present.pdf", false),
        ("pdfua1-rule-7-2-24-language-missing.pdf", true),
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
