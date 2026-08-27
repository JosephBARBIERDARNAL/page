use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_pdf_bytes};

pub mod common;

const RULE: &str = "PDFUA1-HEADING-NESTING-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.4.2:1";

#[test]
fn pdfua1_rule_7_4_2_1_requires_numbered_headings_to_follow_the_previous_level() {
    let valid = validate_pdf_bytes(
        include_bytes!("fixtures/pdfua1-rule-7-4-2-1-valid.pdf"),
        Some(ValidationProfile::PdfUa1),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");
    assert!(valid.is_compliant, "{valid}");
    assert!(valid.failures.is_empty());

    for (fixture, bytes) in [
        (
            "first_heading_h2",
            include_bytes!("fixtures/pdfua1-rule-7-4-2-1-first-heading-h2.pdf").as_slice(),
        ),
        (
            "skips_h2",
            include_bytes!("fixtures/pdfua1-rule-7-4-2-1-skips-h2.pdf").as_slice(),
        ),
    ] {
        let report = validate_pdf_bytes(
            bytes,
            Some(ValidationProfile::PdfUa1),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert!(!report.is_compliant, "{fixture}: {report}");
        assert_eq!(report.checks.failed, 1, "{fixture}: {report}");
        assert_eq!(report.failures.len(), 1, "{fixture}: {report}");
        assert_eq!(report.failures[0].rule_id, RULE, "{fixture}: {report}");
    }
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.4.2-1 fixtures"]
fn regenerate_pdfua1_rule_7_4_2_1_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-4-2-1-valid.pdf", "valid"),
        (
            "pdfua1-rule-7-4-2-1-first-heading-h2.pdf",
            "first_heading_h2",
        ),
        ("pdfua1-rule-7-4-2-1-skips-h2.pdf", "skips_h2"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_4_2_1_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.4.2-1 fixture");
    }
}

#[test]
fn pdfua1_rule_7_4_2_1_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-4-2-1-valid.pdf", false),
        ("pdfua1-rule-7-4-2-1-first-heading-h2.pdf", true),
        ("pdfua1-rule-7-4-2-1-skips-h2.pdf", true),
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
