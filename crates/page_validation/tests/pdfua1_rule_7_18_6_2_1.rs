#![expect(
    clippy::panic,
    reason = "fixture dispatch deliberately fails loudly for an undeclared test case"
)]

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_pdf_bytes};

pub mod common;

const RULE: &str = "PDFUA1-MEDIA-CLIP-CT-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.18.6.2:1";

#[test]
fn pdfua1_rule_7_18_6_2_1_requires_media_clip_ct() {
    let allowed = validate_pdf_bytes(
        fixture_bytes("allowed"),
        Some(ValidationProfile::PdfUa1),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");
    assert!(allowed.is_compliant, "{allowed}");
    assert!(allowed.failures.is_empty(), "{allowed}");

    let invalid = validate_pdf_bytes(
        fixture_bytes("missing_ct"),
        Some(ValidationProfile::PdfUa1),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");
    assert!(!invalid.is_compliant, "{invalid}");
    assert_eq!(invalid.checks.failed, 1, "{invalid}");
    assert_eq!(invalid.failures.len(), 1, "{invalid}");
    assert_eq!(invalid.failures[0].rule_id, RULE, "{invalid}");
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.18.6.2-1 fixtures"]
fn regenerate_pdfua1_rule_7_18_6_2_1_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-18-6-2-1-allowed.pdf", "allowed"),
        ("pdfua1-rule-7-18-6-2-1-missing-ct.pdf", "missing_ct"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_18_6_2_1_fixture(case),
        )
        .expect("write PDF/UA-1 media clip fixture");
    }
}

#[test]
fn pdfua1_rule_7_18_6_2_1_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-18-6-2-1-allowed.pdf", false),
        ("pdfua1-rule-7-18-6-2-1-missing-ct.pdf", true),
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

fn fixture_bytes(case: &str) -> &'static [u8] {
    match case {
        "allowed" => include_bytes!("fixtures/pdfua1-rule-7-18-6-2-1-allowed.pdf"),
        "missing_ct" => include_bytes!("fixtures/pdfua1-rule-7-18-6-2-1-missing-ct.pdf"),
        _ => panic!("unknown fixture {case}"),
    }
}
