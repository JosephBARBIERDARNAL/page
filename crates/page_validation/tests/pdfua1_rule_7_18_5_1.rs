use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-LINK-LINK-TAG-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.18.5:1";

#[test]
fn pdfua1_rule_7_18_5_1_requires_links_inside_link_tags() {
    for fixture in [
        "pdfua1-rule-7-18-5-1-allowed.pdf",
        "pdfua1-rule-7-18-5-1-role-mapped.pdf",
        "pdfua1-rule-7-18-5-1-hidden.pdf",
        "pdfua1-rule-7-18-5-1-outside-crop-box.pdf",
    ] {
        let report = validate_bytes_with_profile(
            fixture_bytes(fixture),
            ValidationProfile::PdfUa1,
            &SafetyLimits::default(),
        );
        assert!(report.checks_passed, "{fixture}: {report}");
        assert!(report.failures.is_empty(), "{fixture}: {report}");
        assert_eq!(report.checks.total, 90, "{fixture}: {report}");
    }

    let invalid = validate_bytes_with_profile(
        fixture_bytes("pdfua1-rule-7-18-5-1-not-nested.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!invalid.checks_passed, "{invalid}");
    assert_eq!(invalid.checks.failed, 1, "{invalid}");
    assert_eq!(invalid.failures.len(), 1, "{invalid}");
    assert_eq!(invalid.failures[0].rule_id, RULE, "{invalid}");
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.18.5-1 fixtures"]
fn regenerate_pdfua1_rule_7_18_5_1_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-18-5-1-allowed.pdf", "allowed"),
        ("pdfua1-rule-7-18-5-1-role-mapped.pdf", "role_mapped"),
        ("pdfua1-rule-7-18-5-1-hidden.pdf", "hidden"),
        (
            "pdfua1-rule-7-18-5-1-outside-crop-box.pdf",
            "outside_crop_box",
        ),
        ("pdfua1-rule-7-18-5-1-not-nested.pdf", "not_nested"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_18_5_1_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.18.5-1 fixture");
    }
}

#[test]
fn pdfua1_rule_7_18_5_1_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-18-5-1-allowed.pdf", false),
        ("pdfua1-rule-7-18-5-1-role-mapped.pdf", false),
        ("pdfua1-rule-7-18-5-1-hidden.pdf", false),
        ("pdfua1-rule-7-18-5-1-outside-crop-box.pdf", false),
        ("pdfua1-rule-7-18-5-1-not-nested.pdf", true),
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

fn fixture_bytes(fixture: &str) -> &'static [u8] {
    match fixture {
        "pdfua1-rule-7-18-5-1-allowed.pdf" => {
            include_bytes!("fixtures/pdfua1-rule-7-18-5-1-allowed.pdf")
        }
        "pdfua1-rule-7-18-5-1-role-mapped.pdf" => {
            include_bytes!("fixtures/pdfua1-rule-7-18-5-1-role-mapped.pdf")
        }
        "pdfua1-rule-7-18-5-1-hidden.pdf" => {
            include_bytes!("fixtures/pdfua1-rule-7-18-5-1-hidden.pdf")
        }
        "pdfua1-rule-7-18-5-1-outside-crop-box.pdf" => {
            include_bytes!("fixtures/pdfua1-rule-7-18-5-1-outside-crop-box.pdf")
        }
        "pdfua1-rule-7-18-5-1-not-nested.pdf" => {
            include_bytes!("fixtures/pdfua1-rule-7-18-5-1-not-nested.pdf")
        }
        _ => panic!("unknown fixture {fixture}"),
    }
}
