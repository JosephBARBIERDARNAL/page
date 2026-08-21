use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-OPTIONAL-CONTENT-NAME-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.10:1";

#[test]
fn pdfua1_rule_7_10_1_requires_names_for_default_and_named_configurations() {
    let valid = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-10-1-valid.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(valid.checks_passed, "{valid}");
    assert!(valid.failures.is_empty());

    for fixture in [
        "pdfua1-rule-7-10-1-missing-default-name.pdf",
        "pdfua1-rule-7-10-1-missing-config-name.pdf",
    ] {
        let bytes = match fixture {
            "pdfua1-rule-7-10-1-missing-default-name.pdf" => {
                include_bytes!("fixtures/pdfua1-rule-7-10-1-missing-default-name.pdf").as_slice()
            }
            "pdfua1-rule-7-10-1-missing-config-name.pdf" => {
                include_bytes!("fixtures/pdfua1-rule-7-10-1-missing-config-name.pdf").as_slice()
            }
            _ => unreachable!(),
        };
        let report =
            validate_bytes_with_profile(bytes, ValidationProfile::PdfUa1, &SafetyLimits::default());
        assert!(!report.checks_passed, "{fixture}: {report}");
        assert_eq!(report.checks.total, 74, "{fixture}: {report}");
        assert_eq!(report.checks.failed, 1, "{fixture}: {report}");
        assert_eq!(report.failures.len(), 1, "{fixture}: {report}");
        assert_eq!(report.failures[0].rule_id, RULE, "{fixture}: {report}");
    }
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.10-1 fixtures"]
fn regenerate_pdfua1_rule_7_10_1_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-10-1-valid.pdf", "valid"),
        (
            "pdfua1-rule-7-10-1-missing-default-name.pdf",
            "missing_default_name",
        ),
        (
            "pdfua1-rule-7-10-1-missing-config-name.pdf",
            "missing_config_name",
        ),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_10_1_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.10-1 fixture");
    }
}

#[test]
fn pdfua1_rule_7_10_1_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-10-1-valid.pdf", false),
        ("pdfua1-rule-7-10-1-missing-default-name.pdf", true),
        ("pdfua1-rule-7-10-1-missing-config-name.pdf", true),
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
