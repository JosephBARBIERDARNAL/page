use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-PRINTER-MARK-ARTIFACT-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.18.8:1";

#[test]
fn pdfua1_rule_7_18_8_1_requires_printer_marks_to_be_artifacts() {
    for case in ["allowed", "hidden", "outside_crop_box"] {
        let report = validate_bytes_with_profile(
            fixture_bytes(case),
            ValidationProfile::PdfUa1,
            &SafetyLimits::default(),
        );
        assert!(report.checks_passed, "{case}: {report}");
        assert!(report.failures.is_empty(), "{case}: {report}");
        assert_eq!(report.checks.total, 86, "{case}: {report}");
    }

    let included = validate_bytes_with_profile(
        fixture_bytes("included"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!included.checks_passed, "{included}");
    assert_eq!(included.checks.failed, 1, "{included}");
    assert_eq!(included.failures.len(), 1, "{included}");
    assert_eq!(included.failures[0].rule_id, RULE, "{included}");
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.18.8-1 fixtures"]
fn regenerate_pdfua1_rule_7_18_8_1_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-18-8-1-allowed.pdf", "allowed"),
        ("pdfua1-rule-7-18-8-1-hidden.pdf", "hidden"),
        (
            "pdfua1-rule-7-18-8-1-outside-crop-box.pdf",
            "outside_crop_box",
        ),
        ("pdfua1-rule-7-18-8-1-included.pdf", "included"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_18_8_1_fixture(case),
        )
        .expect("write PDF/UA-1 PrinterMark fixture");
    }
}

#[test]
fn pdfua1_rule_7_18_8_1_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-18-8-1-allowed.pdf", false),
        ("pdfua1-rule-7-18-8-1-hidden.pdf", false),
        ("pdfua1-rule-7-18-8-1-outside-crop-box.pdf", false),
        ("pdfua1-rule-7-18-8-1-included.pdf", true),
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
        "allowed" => include_bytes!("fixtures/pdfua1-rule-7-18-8-1-allowed.pdf"),
        "hidden" => include_bytes!("fixtures/pdfua1-rule-7-18-8-1-hidden.pdf"),
        "outside_crop_box" => {
            include_bytes!("fixtures/pdfua1-rule-7-18-8-1-outside-crop-box.pdf")
        }
        "included" => include_bytes!("fixtures/pdfua1-rule-7-18-8-1-included.pdf"),
        _ => panic!("unknown fixture {case}"),
    }
}
