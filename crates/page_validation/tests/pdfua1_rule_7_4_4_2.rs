use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-HEADING-STRUCTURE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.4.4:2";

#[test]
fn pdfua1_rule_7_4_4_2_rejects_documents_mixing_h_and_numbered_headings() {
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-4-4-2-h-only.pdf", false),
        ("pdfua1-rule-7-4-4-2-hn-only.pdf", false),
        ("pdfua1-rule-7-4-4-2-h-then-hn.pdf", true),
        ("pdfua1-rule-7-4-4-2-hn-then-h.pdf", true),
    ] {
        let bytes = match fixture {
            "pdfua1-rule-7-4-4-2-h-only.pdf" => {
                include_bytes!("fixtures/pdfua1-rule-7-4-4-2-h-only.pdf").as_slice()
            }
            "pdfua1-rule-7-4-4-2-hn-only.pdf" => {
                include_bytes!("fixtures/pdfua1-rule-7-4-4-2-hn-only.pdf").as_slice()
            }
            "pdfua1-rule-7-4-4-2-h-then-hn.pdf" => {
                include_bytes!("fixtures/pdfua1-rule-7-4-4-2-h-then-hn.pdf").as_slice()
            }
            "pdfua1-rule-7-4-4-2-hn-then-h.pdf" => {
                include_bytes!("fixtures/pdfua1-rule-7-4-4-2-hn-then-h.pdf").as_slice()
            }
            _ => unreachable!(),
        };
        let report =
            validate_bytes_with_profile(bytes, ValidationProfile::PdfUa1, &SafetyLimits::default());
        assert_eq!(report.checks_passed, !should_fail, "{fixture}: {report}");
        assert_eq!(
            report.failures.len(),
            usize::from(should_fail),
            "{fixture}: {report}"
        );
        if should_fail {
            assert_eq!(report.failures[0].rule_id, RULE, "{fixture}: {report}");
        }
    }
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.4.4-2 fixtures"]
fn regenerate_pdfua1_rule_7_4_4_2_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-4-4-2-h-only.pdf", "h_only"),
        ("pdfua1-rule-7-4-4-2-hn-only.pdf", "hn_only"),
        ("pdfua1-rule-7-4-4-2-h-then-hn.pdf", "h_then_hn"),
        ("pdfua1-rule-7-4-4-2-hn-then-h.pdf", "hn_then_h"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_4_4_2_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.4.4-2 fixture");
    }
}

#[test]
fn pdfua1_rule_7_4_4_2_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-4-4-2-h-only.pdf", false),
        ("pdfua1-rule-7-4-4-2-hn-only.pdf", false),
        ("pdfua1-rule-7-4-4-2-h-then-hn.pdf", true),
        ("pdfua1-rule-7-4-4-2-hn-then-h.pdf", true),
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
