use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes};

pub mod common;

const RULE: &str = "PDFUA1-FIGURE-ALTERNATIVE-TEXT-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.3:1";

#[test]
fn pdfua1_rule_7_3_1_requires_figure_alternative_text() {
    for fixture in [
        "pdfua1-rule-7-3-1-alt-present.pdf",
        "pdfua1-rule-7-3-1-actual-text-present.pdf",
    ] {
        let bytes = match fixture {
            "pdfua1-rule-7-3-1-alt-present.pdf" => {
                include_bytes!("fixtures/pdfua1-rule-7-3-1-alt-present.pdf").as_slice()
            }
            "pdfua1-rule-7-3-1-actual-text-present.pdf" => {
                include_bytes!("fixtures/pdfua1-rule-7-3-1-actual-text-present.pdf").as_slice()
            }
            _ => panic!("unknown PDF/UA-1 rule 7.3.1 fixture {fixture}"),
        };
        let report = validate_bytes(
            bytes,
            Some(ValidationProfile::PdfUa1),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert!(report.checks_passed, "{fixture}: {report}");
    }
    for (fixture, bytes) in [
        (
            "pdfua1-rule-7-3-1-alt-empty.pdf",
            include_bytes!("fixtures/pdfua1-rule-7-3-1-alt-empty.pdf").as_slice(),
        ),
        (
            "pdfua1-rule-7-3-1-missing.pdf",
            include_bytes!("fixtures/pdfua1-rule-7-3-1-missing.pdf").as_slice(),
        ),
    ] {
        let report = validate_bytes(
            bytes,
            Some(ValidationProfile::PdfUa1),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert!(!report.checks_passed, "{fixture}: {report}");
        assert_eq!(report.checks.failed, 1, "{fixture}: {report}");
        assert_eq!(report.failures.len(), 1, "{fixture}: {report}");
        assert_eq!(report.failures[0].rule_id, RULE, "{fixture}: {report}");
    }
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.3-1 fixtures"]
fn regenerate_pdfua1_rule_7_3_1_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-3-1-alt-present.pdf", "alt_present"),
        ("pdfua1-rule-7-3-1-alt-empty.pdf", "alt_empty"),
        (
            "pdfua1-rule-7-3-1-actual-text-present.pdf",
            "actual_text_present",
        ),
        ("pdfua1-rule-7-3-1-missing.pdf", "missing"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_3_1_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.3-1 fixture");
    }
}

#[test]
fn pdfua1_rule_7_3_1_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-3-1-alt-present.pdf", false),
        ("pdfua1-rule-7-3-1-actual-text-present.pdf", false),
        ("pdfua1-rule-7-3-1-alt-empty.pdf", true),
        ("pdfua1-rule-7-3-1-missing.pdf", true),
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
