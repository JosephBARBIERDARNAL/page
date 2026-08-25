use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-TABLE-CAPTION-POSITION-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:16";

#[test]
fn pdfua1_rule_7_2_16_allows_caption_as_first_or_last_table_kid() {
    for (fixture, bytes) in [
        (
            "pdfua1-rule-7-2-16-caption-first.pdf",
            include_bytes!("fixtures/pdfua1-rule-7-2-16-caption-first.pdf"),
        ),
        (
            "pdfua1-rule-7-2-16-caption-last.pdf",
            include_bytes!("fixtures/pdfua1-rule-7-2-16-caption-last.pdf"),
        ),
    ] {
        let report =
            validate_bytes_with_profile(bytes, ValidationProfile::PdfUa1, &SafetyLimits::default());
        assert!(report.checks_passed, "{fixture}: {report}");
        assert!(report.failures.is_empty(), "{fixture}: {report}");
    }

    let invalid = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-2-16-caption-middle.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!invalid.checks_passed, "{invalid}");
    assert_eq!(invalid.checks.failed, 1);
    assert_eq!(invalid.failures.len(), 1);
    assert_eq!(invalid.failures[0].rule_id, RULE);
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.2-16 fixtures"]
fn regenerate_pdfua1_rule_7_2_16_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-2-16-caption-first.pdf", "caption_first"),
        ("pdfua1-rule-7-2-16-caption-last.pdf", "caption_last"),
        ("pdfua1-rule-7-2-16-caption-middle.pdf", "caption_middle"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_2_16_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.2-16 fixture");
    }
}

#[test]
fn pdfua1_rule_7_2_16_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-2-16-caption-first.pdf", false),
        ("pdfua1-rule-7-2-16-caption-last.pdf", false),
        ("pdfua1-rule-7-2-16-caption-middle.pdf", true),
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
