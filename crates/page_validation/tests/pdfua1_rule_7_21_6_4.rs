use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-TRUETYPE-SYMBOLIC-CMAP-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.6:4";

#[test]
fn pdfua1_rule_7_21_6_4_requires_a_single_or_symbol_cmap_for_symbolic_truetype() {
    for (fixture, expected_rule) in [
        ("one_cmap", None),
        ("two_cmaps", Some(RULE)),
        ("two_cmaps_with_cmap30", None),
    ] {
        let report = validate_bytes_with_profile(
            fixture_bytes(fixture),
            ValidationProfile::PdfUa1,
            &SafetyLimits::default(),
        );
        assert_eq!(report.checks.total, 91, "{fixture}: {report}");
        match expected_rule {
            Some(rule) => assert!(
                report
                    .failures
                    .iter()
                    .any(|failure| failure.rule_id == rule),
                "{fixture}: {report}"
            ),
            None => assert!(report.checks_passed, "{fixture}: {report}"),
        }
    }
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.21.6-4 fixtures"]
fn regenerate_pdfua1_rule_7_21_6_4_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-21-6-4-one-cmap.pdf", "one_cmap"),
        ("pdfua1-rule-7-21-6-4-two-cmaps.pdf", "two_cmaps"),
        (
            "pdfua1-rule-7-21-6-4-two-cmaps-with-cmap30.pdf",
            "two_cmaps_with_cmap30",
        ),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_21_6_4_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.21.6-4 fixture");
    }
}

#[test]
fn pdfua1_rule_7_21_6_4_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-21-6-4-one-cmap.pdf", false),
        ("pdfua1-rule-7-21-6-4-two-cmaps.pdf", true),
        ("pdfua1-rule-7-21-6-4-two-cmaps-with-cmap30.pdf", false),
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
        "one_cmap" => include_bytes!("fixtures/pdfua1-rule-7-21-6-4-one-cmap.pdf"),
        "two_cmaps" => include_bytes!("fixtures/pdfua1-rule-7-21-6-4-two-cmaps.pdf"),
        "two_cmaps_with_cmap30" => {
            include_bytes!("fixtures/pdfua1-rule-7-21-6-4-two-cmaps-with-cmap30.pdf")
        }
        _ => panic!("unknown PDF/UA-1 rule 7.21.6-4 fixture case {fixture}"),
    }
}
