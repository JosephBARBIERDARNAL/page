use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-ANNOTATION-CONTENTS-ALT-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.18.1:2";

#[test]
fn pdfua1_rule_7_18_1_2_requires_annotation_contents_or_structure_alt() {
    for fixture in [
        "pdfua1-rule-7-18-1-2-contents.pdf",
        "pdfua1-rule-7-18-1-2-alt.pdf",
    ] {
        let report = validate_bytes_with_profile(
            fixture_bytes(fixture),
            ValidationProfile::PdfUa1,
            &SafetyLimits::default(),
        );
        assert!(report.checks_passed, "{fixture}: {report}");
        assert!(report.failures.is_empty(), "{fixture}: {report}");
        assert_eq!(report.checks.total, 74, "{fixture}: {report}");
    }

    for fixture in [
        "pdfua1-rule-7-18-1-2-missing.pdf",
        "pdfua1-rule-7-18-1-2-empty-contents.pdf",
    ] {
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
#[ignore = "maintenance generator for PDF/UA-1 rule 7.18.1-2 fixtures"]
fn regenerate_pdfua1_rule_7_18_1_2_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-18-1-2-contents.pdf", "contents"),
        ("pdfua1-rule-7-18-1-2-alt.pdf", "alt"),
        ("pdfua1-rule-7-18-1-2-missing.pdf", "missing"),
        ("pdfua1-rule-7-18-1-2-empty-contents.pdf", "empty_contents"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_18_1_2_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.18.1-2 fixture");
    }
}

#[test]
fn pdfua1_rule_7_18_1_2_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-18-1-2-contents.pdf", false),
        ("pdfua1-rule-7-18-1-2-alt.pdf", false),
        ("pdfua1-rule-7-18-1-2-missing.pdf", true),
        ("pdfua1-rule-7-18-1-2-empty-contents.pdf", true),
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
        "pdfua1-rule-7-18-1-2-contents.pdf" => {
            include_bytes!("fixtures/pdfua1-rule-7-18-1-2-contents.pdf")
        }
        "pdfua1-rule-7-18-1-2-alt.pdf" => include_bytes!("fixtures/pdfua1-rule-7-18-1-2-alt.pdf"),
        "pdfua1-rule-7-18-1-2-missing.pdf" => {
            include_bytes!("fixtures/pdfua1-rule-7-18-1-2-missing.pdf")
        }
        "pdfua1-rule-7-18-1-2-empty-contents.pdf" => {
            include_bytes!("fixtures/pdfua1-rule-7-18-1-2-empty-contents.pdf")
        }
        _ => panic!("unknown fixture {fixture}"),
    }
}
