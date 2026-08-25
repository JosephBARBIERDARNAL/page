use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-CIDTOGIDMAP-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.3.2:1";

#[test]
fn pdfua1_rule_7_21_3_2_requires_embedded_type2_cidfonts_to_define_cid_to_gid_map() {
    for fixture in [
        "pdfua1-rule-7-21-3-2-identity.pdf",
        "pdfua1-rule-7-21-3-2-stream.pdf",
    ] {
        let report = validate_bytes_with_profile(
            fixture_bytes(fixture),
            ValidationProfile::PdfUa1,
            &SafetyLimits::default(),
        );
        assert!(report.checks_passed, "{fixture}: {report}");
        assert!(report.failures.is_empty(), "{fixture}: {report}");
        assert_eq!(report.checks.total, 91, "{fixture}: {report}");
    }

    for fixture in [
        "pdfua1-rule-7-21-3-2-missing.pdf",
        "pdfua1-rule-7-21-3-2-invalid.pdf",
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
        "pdfua1-rule-7-21-3-2-identity.pdf" => {
            include_bytes!("fixtures/pdfua1-rule-7-21-3-2-identity.pdf")
        }
        "pdfua1-rule-7-21-3-2-stream.pdf" => {
            include_bytes!("fixtures/pdfua1-rule-7-21-3-2-stream.pdf")
        }
        "pdfua1-rule-7-21-3-2-missing.pdf" => {
            include_bytes!("fixtures/pdfua1-rule-7-21-3-2-missing.pdf")
        }
        "pdfua1-rule-7-21-3-2-invalid.pdf" => {
            include_bytes!("fixtures/pdfua1-rule-7-21-3-2-invalid.pdf")
        }
        _ => panic!("unknown PDF/UA-1 rule 7.21.3.2-1 fixture {fixture}"),
    }
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.21.3.2-1 fixtures"]
fn regenerate_pdfua1_rule_7_21_3_2_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-21-3-2-identity.pdf", "identity"),
        ("pdfua1-rule-7-21-3-2-stream.pdf", "stream"),
        ("pdfua1-rule-7-21-3-2-missing.pdf", "missing"),
        ("pdfua1-rule-7-21-3-2-invalid.pdf", "invalid"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_21_3_2_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.21.3.2-1 fixture");
    }
}

#[test]
fn pdfua1_rule_7_21_3_2_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-21-3-2-identity.pdf", false),
        ("pdfua1-rule-7-21-3-2-stream.pdf", false),
        ("pdfua1-rule-7-21-3-2-missing.pdf", true),
        ("pdfua1-rule-7-21-3-2-invalid.pdf", true),
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
