use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-TRUETYPE-SYMBOLIC-ENCODING-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.6:3";

#[test]
fn pdfua1_rule_7_21_6_3_rejects_symbolic_truetype_encoding_entries() {
    let matching = validate_bytes_with_profile(
        fixture_bytes("matching"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(matching.checks_passed, "{matching}");
    assert!(matching.failures.is_empty(), "{matching}");
    assert_eq!(matching.checks.total, 89, "{matching}");

    let encoding = validate_bytes_with_profile(
        fixture_bytes("encoding"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!encoding.checks_passed, "{encoding}");
    assert_eq!(encoding.checks.failed, 1, "{encoding}");
    assert_eq!(encoding.failures.len(), 1, "{encoding}");
    assert_eq!(encoding.failures[0].rule_id, RULE, "{encoding}");
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.21.6-3 fixtures"]
fn regenerate_pdfua1_rule_7_21_6_3_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-21-6-3-matching.pdf", "matching"),
        ("pdfua1-rule-7-21-6-3-encoding.pdf", "encoding"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_21_6_3_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.21.6-3 fixture");
    }
}

#[test]
fn pdfua1_rule_7_21_6_3_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-21-6-3-matching.pdf", false),
        ("pdfua1-rule-7-21-6-3-encoding.pdf", true),
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

fn fixture_bytes(case: &str) -> &'static [u8] {
    match case {
        "matching" => include_bytes!("fixtures/pdfua1-rule-7-21-6-3-matching.pdf"),
        "encoding" => include_bytes!("fixtures/pdfua1-rule-7-21-6-3-encoding.pdf"),
        _ => panic!("unknown PDF/UA-1 rule 7.21.6-3 fixture case {case}"),
    }
}
