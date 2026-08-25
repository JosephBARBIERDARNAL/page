use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-FONT-UNICODE-VALUE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.7:2";

#[test]
fn pdfua1_rule_7_21_7_2_rejects_reserved_unicode_values() {
    let matching = validate_bytes_with_profile(
        fixture_bytes("matching"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(matching.checks_passed, "{matching}");
    assert!(matching.failures.is_empty(), "{matching}");

    for fixture in ["zero", "feff", "fffe"] {
        let report = validate_bytes_with_profile(
            fixture_bytes(fixture),
            ValidationProfile::PdfUa1,
            &SafetyLimits::default(),
        );
        assert!(!report.checks_passed, "{fixture}: {report}");
        assert_eq!(report.checks.failed, 1, "{fixture}: {report}");
        assert_eq!(report.failures.len(), 1, "{fixture}: {report}");
        assert_eq!(report.failures[0].rule_id, RULE, "{fixture}: {report}");
    }
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.21.7-2 fixtures"]
fn regenerate_pdfua1_rule_7_21_7_2_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-21-7-2-matching.pdf", "matching"),
        ("pdfua1-rule-7-21-7-2-zero.pdf", "zero"),
        ("pdfua1-rule-7-21-7-2-feff.pdf", "feff"),
        ("pdfua1-rule-7-21-7-2-fffe.pdf", "fffe"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_21_7_2_fixture(case),
        )
        .expect("write PDF/UA-1 Unicode value fixture");
    }
}

#[test]
fn pdfua1_rule_7_21_7_2_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-21-7-2-matching.pdf", false),
        ("pdfua1-rule-7-21-7-2-zero.pdf", true),
        ("pdfua1-rule-7-21-7-2-feff.pdf", true),
        ("pdfua1-rule-7-21-7-2-fffe.pdf", true),
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
        assert!(report.operational_failure.is_none(), "{fixture}: {report}");
    }
}

fn fixture_bytes(fixture: &str) -> &'static [u8] {
    match fixture {
        "matching" => include_bytes!("fixtures/pdfua1-rule-7-21-7-2-matching.pdf"),
        "zero" => include_bytes!("fixtures/pdfua1-rule-7-21-7-2-zero.pdf"),
        "feff" => include_bytes!("fixtures/pdfua1-rule-7-21-7-2-feff.pdf"),
        "fffe" => include_bytes!("fixtures/pdfua1-rule-7-21-7-2-fffe.pdf"),
        _ => panic!("unknown PDF/UA-1 rule 7.21.7-2 fixture case {fixture}"),
    }
}
