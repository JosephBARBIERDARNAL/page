#![expect(
    clippy::panic,
    reason = "fixture dispatch deliberately fails loudly for an undeclared test case"
)]

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-FONT-TYPE1-CHARSET-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.4.2:1";

#[test]
fn pdfua1_rule_7_21_4_2_1_requires_charset_to_list_all_type1_program_glyphs() {
    let pass = validate_bytes_with_profile(
        fixture_bytes("complete"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(pass.checks_passed, "{pass}");
    assert!(pass.failures.is_empty(), "{pass}");

    let fail = validate_bytes_with_profile(
        fixture_bytes("incomplete"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!fail.checks_passed, "{fail}");
    assert_eq!(fail.checks.failed, 1, "{fail}");
    assert_eq!(fail.failures.len(), 1, "{fail}");
    assert_eq!(fail.failures[0].rule_id, RULE, "{fail}");
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.21.4.2-1 fixtures"]
fn regenerate_pdfua1_rule_7_21_4_2_1_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-21-4-2-1-complete.pdf", "complete"),
        ("pdfua1-rule-7-21-4-2-1-incomplete.pdf", "incomplete"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_21_4_2_1_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.21.4.2-1 fixture");
    }
}

#[test]
fn pdfua1_rule_7_21_4_2_1_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-21-4-2-1-complete.pdf", false),
        ("pdfua1-rule-7-21-4-2-1-incomplete.pdf", true),
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
        "complete" => include_bytes!("fixtures/pdfua1-rule-7-21-4-2-1-complete.pdf"),
        "incomplete" => include_bytes!("fixtures/pdfua1-rule-7-21-4-2-1-incomplete.pdf"),
        _ => panic!("unknown PDF/UA-1 rule 7.21.4.2-1 fixture case {fixture}"),
    }
}
