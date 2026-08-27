#![expect(
    clippy::panic,
    reason = "fixture dispatch deliberately fails loudly for an undeclared test case"
)]

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_pdf_bytes};

pub mod common;

const RULE: &str = "PDFUA1-CMAP-EMBEDDING-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.3.3:1";

#[test]
fn pdfua1_rule_7_21_3_3_1_requires_nonstandard_cmaps_to_be_embedded() {
    let fixture = "pdfua1-rule-7-21-3-3-1-embedded.pdf";
    {
        let report = validate_pdf_bytes(
            fixture_bytes(fixture),
            Some(ValidationProfile::PdfUa1),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert!(report.is_compliant, "{fixture}: {report}");
        assert!(report.failures.is_empty(), "{fixture}: {report}");
    }

    let fixture = "pdfua1-rule-7-21-3-3-1-predefined.pdf";
    let report = validate_pdf_bytes(
        fixture_bytes(fixture),
        Some(ValidationProfile::PdfUa1),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");
    assert!(!report.is_compliant, "{fixture}: {report}");
    assert_eq!(report.checks.failed, 1, "{fixture}: {report}");
    assert_eq!(report.failures.len(), 1, "{fixture}: {report}");
    assert_eq!(
        report.failures[0].rule_id, "PDFUA1-FONT-GLYPH-PRESENCE-001",
        "{fixture}: {report}"
    );

    let fixture = "pdfua1-rule-7-21-3-3-1-unembedded.pdf";
    let report = validate_pdf_bytes(
        fixture_bytes(fixture),
        Some(ValidationProfile::PdfUa1),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");
    assert!(!report.is_compliant, "{fixture}: {report}");
    assert_eq!(report.checks.failed, 1, "{fixture}: {report}");
    assert_eq!(report.failures.len(), 1, "{fixture}: {report}");
    assert_eq!(report.failures[0].rule_id, RULE, "{fixture}: {report}");
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.21.3.3-1 fixtures"]
fn regenerate_pdfua1_rule_7_21_3_3_1_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-21-3-3-1-embedded.pdf", "embedded"),
        ("pdfua1-rule-7-21-3-3-1-predefined.pdf", "predefined"),
        ("pdfua1-rule-7-21-3-3-1-unembedded.pdf", "unembedded"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_21_3_3_1_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.21.3.3-1 fixture");
    }
}

#[test]
fn pdfua1_rule_7_21_3_3_1_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-21-3-3-1-embedded.pdf", false),
        ("pdfua1-rule-7-21-3-3-1-predefined.pdf", false),
        ("pdfua1-rule-7-21-3-3-1-unembedded.pdf", true),
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
        "pdfua1-rule-7-21-3-3-1-embedded.pdf" => {
            include_bytes!("fixtures/pdfua1-rule-7-21-3-3-1-embedded.pdf")
        }
        "pdfua1-rule-7-21-3-3-1-predefined.pdf" => {
            include_bytes!("fixtures/pdfua1-rule-7-21-3-3-1-predefined.pdf")
        }
        "pdfua1-rule-7-21-3-3-1-unembedded.pdf" => {
            include_bytes!("fixtures/pdfua1-rule-7-21-3-3-1-unembedded.pdf")
        }
        _ => panic!("unknown PDF/UA-1 rule 7.21.3.3-1 fixture {fixture}"),
    }
}
