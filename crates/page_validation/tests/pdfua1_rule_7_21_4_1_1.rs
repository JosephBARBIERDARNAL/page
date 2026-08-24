use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-FONT-EMBEDDING-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.4.1:1";

#[test]
fn pdfua1_rule_7_21_4_1_1_requires_rendered_font_programs_to_be_embedded() {
    let embedded = validate_bytes_with_profile(
        fixture_bytes("embedded"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(embedded.checks_passed, "{embedded}");
    assert!(embedded.failures.is_empty(), "{embedded}");
    assert_eq!(embedded.checks.total, 87, "{embedded}");

    let unembedded = validate_bytes_with_profile(
        fixture_bytes("unembedded"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!unembedded.checks_passed, "{unembedded}");
    assert_eq!(unembedded.checks.failed, 1, "{unembedded}");
    assert_eq!(unembedded.failures.len(), 1, "{unembedded}");
    assert_eq!(unembedded.failures[0].rule_id, RULE, "{unembedded}");
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.21.4.1-1 fixtures"]
fn regenerate_pdfua1_rule_7_21_4_1_1_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-21-4-1-1-embedded.pdf", "embedded"),
        ("pdfua1-rule-7-21-4-1-1-unembedded.pdf", "unembedded"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_21_4_1_1_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.21.4.1-1 fixture");
    }
}

#[test]
fn pdfua1_rule_7_21_4_1_1_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-21-4-1-1-embedded.pdf", false),
        ("pdfua1-rule-7-21-4-1-1-unembedded.pdf", true),
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
        "embedded" => include_bytes!("fixtures/pdfua1-rule-7-21-4-1-1-embedded.pdf"),
        "unembedded" => include_bytes!("fixtures/pdfua1-rule-7-21-4-1-1-unembedded.pdf"),
        _ => panic!("unknown PDF/UA-1 rule 7.21.4.1-1 fixture case {fixture}"),
    }
}
