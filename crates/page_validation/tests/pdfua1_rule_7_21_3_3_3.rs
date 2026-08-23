use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-CMAP-REFERENCE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.3.3:3";

#[test]
fn pdfua1_rule_7_21_3_3_3_allows_table_118_cmaps_and_rejects_other_references() {
    let allowed = validate_bytes_with_profile(
        fixture_bytes("allowed"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(allowed.checks_passed, "{allowed}");
    assert!(allowed.failures.is_empty(), "{allowed}");
    assert_eq!(allowed.checks.total, 80, "{allowed}");

    for fixture in ["embedded_unknown", "dictionary_unknown"] {
        let report = validate_bytes_with_profile(
            fixture_bytes(fixture),
            ValidationProfile::PdfUa1,
            &SafetyLimits::default(),
        );
        assert!(!report.checks_passed, "{fixture}: {report}");
        assert_eq!(report.checks.total, 80, "{fixture}: {report}");
        assert_eq!(report.checks.failed, 1, "{fixture}: {report}");
        assert_eq!(report.failures.len(), 1, "{fixture}: {report}");
        assert_eq!(report.failures[0].rule_id, RULE, "{fixture}: {report}");
    }
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.21.3.3-3 fixtures"]
fn regenerate_pdfua1_rule_7_21_3_3_3_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-21-3-3-3-allowed.pdf", "allowed"),
        (
            "pdfua1-rule-7-21-3-3-3-embedded-unknown.pdf",
            "embedded_unknown",
        ),
        (
            "pdfua1-rule-7-21-3-3-3-dictionary-unknown.pdf",
            "dictionary_unknown",
        ),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_21_3_3_3_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.21.3.3-3 fixture");
    }
}

#[test]
fn pdfua1_rule_7_21_3_3_3_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    // veraPDF 1.30.2 catches the dictionary /UseCMap form, but does not
    // expose the stream-level usecmap operator in this rule. The local check
    // follows the published requirement and intentionally covers both forms.
    for (fixture, reference_should_fail) in [
        ("pdfua1-rule-7-21-3-3-3-allowed.pdf", false),
        ("pdfua1-rule-7-21-3-3-3-embedded-unknown.pdf", false),
        ("pdfua1-rule-7-21-3-3-3-dictionary-unknown.pdf", true),
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture);
        let report = runner.compare_file(&path, &SafetyLimits::default());
        let Some(reference_result) = report.reference_result.as_ref() else {
            panic!("{fixture}: {report}");
        };
        let failed = reference_result
            .failed_rule_ids
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            failed.contains(REFERENCE_RULE),
            reference_should_fail,
            "{fixture}: {report}"
        );
        assert!(report.operational_failure.is_none(), "{fixture}: {report}");
    }
}

fn fixture_bytes(fixture: &str) -> &'static [u8] {
    match fixture {
        "allowed" => include_bytes!("fixtures/pdfua1-rule-7-21-3-3-3-allowed.pdf"),
        "embedded_unknown" => {
            include_bytes!("fixtures/pdfua1-rule-7-21-3-3-3-embedded-unknown.pdf")
        }
        "dictionary_unknown" => {
            include_bytes!("fixtures/pdfua1-rule-7-21-3-3-3-dictionary-unknown.pdf")
        }
        _ => panic!("unknown PDF/UA-1 rule 7.21.3.3-3 fixture case {fixture}"),
    }
}
