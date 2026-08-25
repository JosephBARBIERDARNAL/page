use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-NOTDEF-GLYPH-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.8:1";

#[test]
fn pdfua1_rule_7_21_8_1_rejects_notdef_references_even_in_rendering_mode_3() {
    let matching = validate_bytes_with_profile(
        fixture_bytes("matching"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(matching.checks_passed, "{matching}");
    assert!(matching.failures.is_empty(), "{matching}");

    let notdef = validate_bytes_with_profile(
        fixture_bytes("notdef"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!notdef.checks_passed, "{notdef}");
    assert!(
        notdef
            .failures
            .iter()
            .any(|failure| failure.rule_id == RULE),
        "{notdef}"
    );
}

#[test]
fn pdfua1_rule_7_21_8_1_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-21-8-1-matching.pdf", false),
        ("pdfua1-rule-7-21-8-1-notdef.pdf", true),
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

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.21.8-1 fixtures"]
fn regenerate_pdfua1_rule_7_21_8_1_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-21-8-1-matching.pdf", "matching"),
        ("pdfua1-rule-7-21-8-1-notdef.pdf", "fail"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_21_8_1_fixture(case),
        )
        .expect("write PDF/UA-1 .notdef fixture");
    }
}

fn fixture_bytes(fixture: &str) -> &'static [u8] {
    match fixture {
        "matching" => include_bytes!("fixtures/pdfua1-rule-7-21-8-1-matching.pdf"),
        "notdef" => include_bytes!("fixtures/pdfua1-rule-7-21-8-1-notdef.pdf"),
        _ => panic!("unknown PDF/UA-1 rule 7.21.8-1 fixture case {fixture}"),
    }
}
