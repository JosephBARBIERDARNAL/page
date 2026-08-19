use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-TOC-CAPTION-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:28";

#[test]
fn pdfua1_rule_7_2_28_allows_caption_only_as_first_toc_kid() {
    let caption_first = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-2-28-caption-first.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(caption_first.checks_passed, "{caption_first}");
    assert_eq!(caption_first.checks.total, 29);
    assert_eq!(caption_first.checks.passed, 29);
    assert!(caption_first.failures.is_empty());

    let caption_not_first = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-2-28-caption-not-first.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!caption_not_first.checks_passed, "{caption_not_first}");
    assert_eq!(caption_not_first.checks.total, 29);
    assert_eq!(caption_not_first.checks.failed, 1);
    assert_eq!(caption_not_first.failures.len(), 1);
    assert_eq!(caption_not_first.failures[0].rule_id, RULE);
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.2-28 fixtures"]
fn regenerate_pdfua1_rule_7_2_28_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-2-28-caption-first.pdf", "caption_first"),
        (
            "pdfua1-rule-7-2-28-caption-not-first.pdf",
            "caption_not_first",
        ),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_2_28_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.2-28 fixture");
    }
}

#[test]
fn pdfua1_rule_7_2_28_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-2-28-caption-first.pdf", false),
        ("pdfua1-rule-7-2-28-caption-not-first.pdf", true),
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture);
        let report = runner.compare_file(&path, &SafetyLimits::default());
        let failed = report
            .reference_result
            .as_ref()
            .unwrap_or_else(|| panic!("{report}"))
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
