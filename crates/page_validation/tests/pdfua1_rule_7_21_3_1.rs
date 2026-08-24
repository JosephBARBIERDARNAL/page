use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-TYPE0-CID-SYSTEM-INFO-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.3.1:1";

#[test]
fn pdfua1_rule_7_21_3_1_allows_identity_and_checks_nonidentity_system_info() {
    for fixture in [
        "pdfua1-rule-7-21-3-1-identity.pdf",
        "pdfua1-rule-7-21-3-1-matching.pdf",
    ] {
        let report = validate_bytes_with_profile(
            fixture_bytes(fixture),
            ValidationProfile::PdfUa1,
            &SafetyLimits::default(),
        );
        assert!(report.checks_passed, "{fixture}: {report}");
        assert!(report.failures.is_empty(), "{fixture}: {report}");
        assert_eq!(report.checks.total, 87, "{fixture}: {report}");
    }

    for fixture in [
        "pdfua1-rule-7-21-3-1-registry-mismatch.pdf",
        "pdfua1-rule-7-21-3-1-supplement-mismatch.pdf",
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

fn fixture_bytes(fixture: &str) -> &'static [u8] {
    match fixture {
        "pdfua1-rule-7-21-3-1-identity.pdf" => {
            include_bytes!("fixtures/pdfua1-rule-7-21-3-1-identity.pdf")
        }
        "pdfua1-rule-7-21-3-1-matching.pdf" => {
            include_bytes!("fixtures/pdfua1-rule-7-21-3-1-matching.pdf")
        }
        "pdfua1-rule-7-21-3-1-registry-mismatch.pdf" => {
            include_bytes!("fixtures/pdfua1-rule-7-21-3-1-registry-mismatch.pdf")
        }
        "pdfua1-rule-7-21-3-1-supplement-mismatch.pdf" => {
            include_bytes!("fixtures/pdfua1-rule-7-21-3-1-supplement-mismatch.pdf")
        }
        _ => panic!("unknown PDF/UA-1 rule 7.21.3.1-1 fixture {fixture}"),
    }
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.21.3.1-1 fixtures"]
fn regenerate_pdfua1_rule_7_21_3_1_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-21-3-1-identity.pdf", "identity"),
        ("pdfua1-rule-7-21-3-1-matching.pdf", "matching"),
        (
            "pdfua1-rule-7-21-3-1-registry-mismatch.pdf",
            "registry_mismatch",
        ),
        (
            "pdfua1-rule-7-21-3-1-supplement-mismatch.pdf",
            "supplement_mismatch",
        ),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_21_3_1_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.21.3.1-1 fixture");
    }
}

#[test]
fn pdfua1_rule_7_21_3_1_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-21-3-1-identity.pdf", false),
        ("pdfua1-rule-7-21-3-1-matching.pdf", false),
        ("pdfua1-rule-7-21-3-1-registry-mismatch.pdf", true),
        ("pdfua1-rule-7-21-3-1-supplement-mismatch.pdf", true),
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
