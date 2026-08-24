use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-CID-SUBSET-CIDSET-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.4.2:2";

#[test]
fn pdfua1_rule_7_21_4_2_2_requires_cidset_to_list_unreferenced_program_cids() {
    let complete = validate_bytes_with_profile(
        fixture_bytes("complete"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(complete.checks_passed, "{complete}");
    assert!(complete.failures.is_empty(), "{complete}");
    assert_eq!(complete.checks.total, 86, "{complete}");

    let incomplete = validate_bytes_with_profile(
        fixture_bytes("incomplete"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!incomplete.checks_passed, "{incomplete}");
    assert_eq!(incomplete.checks.failed, 1, "{incomplete}");
    assert_eq!(incomplete.failures.len(), 1, "{incomplete}");
    assert_eq!(incomplete.failures[0].rule_id, RULE, "{incomplete}");
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.21.4.2-2 fixtures"]
fn regenerate_pdfua1_rule_7_21_4_2_2_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-21-4-2-2-complete.pdf", "complete"),
        ("pdfua1-rule-7-21-4-2-2-incomplete.pdf", "incomplete"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_21_4_2_2_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.21.4.2-2 fixture");
    }
}

#[test]
fn pdfua1_rule_7_21_4_2_2_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-21-4-2-2-complete.pdf", false),
        ("pdfua1-rule-7-21-4-2-2-incomplete.pdf", true),
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
        "complete" => include_bytes!("fixtures/pdfua1-rule-7-21-4-2-2-complete.pdf"),
        "incomplete" => include_bytes!("fixtures/pdfua1-rule-7-21-4-2-2-incomplete.pdf"),
        _ => panic!("unknown PDF/UA-1 rule 7.21.4.2-2 fixture case {case}"),
    }
}
