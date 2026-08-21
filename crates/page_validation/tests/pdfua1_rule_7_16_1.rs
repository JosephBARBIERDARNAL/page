use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{
    ComparisonClassification, DifferentialRunner, ReferenceConfig, ReferenceProfile,
};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-ENCRYPTION-P-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.16:1";

#[test]
fn pdfua1_rule_7_16_1_requires_p_with_bit_10_set_for_encrypted_files() {
    let valid = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-16-1-valid.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(valid.checks_passed, "{valid}");
    assert!(valid.failures.is_empty(), "{valid}");

    for fixture in [
        "pdfua1-rule-7-16-1-bit-10-false.pdf",
        "pdfua1-rule-7-16-1-missing-p.pdf",
    ] {
        let bytes = match fixture {
            "pdfua1-rule-7-16-1-bit-10-false.pdf" => {
                include_bytes!("fixtures/pdfua1-rule-7-16-1-bit-10-false.pdf").as_slice()
            }
            "pdfua1-rule-7-16-1-missing-p.pdf" => {
                include_bytes!("fixtures/pdfua1-rule-7-16-1-missing-p.pdf").as_slice()
            }
            _ => unreachable!(),
        };
        let report =
            validate_bytes_with_profile(bytes, ValidationProfile::PdfUa1, &SafetyLimits::default());
        assert!(!report.checks_passed, "{fixture}: {report}");
        assert_eq!(report.checks.total, 66, "{fixture}: {report}");
        assert!(
            report
                .failures
                .iter()
                .any(|failure| failure.rule_id == RULE),
            "{fixture}: {report}"
        );
    }
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.16-1 fixtures"]
fn regenerate_pdfua1_rule_7_16_1_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-16-1-valid.pdf", "valid"),
        ("pdfua1-rule-7-16-1-bit-10-false.pdf", "bit_10_false"),
        ("pdfua1-rule-7-16-1-missing-p.pdf", "missing_p"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_16_1_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.16-1 fixture");
    }
}

#[test]
fn pdfua1_rule_7_16_1_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail, reference_parser_discrepancy) in [
        ("pdfua1-rule-7-16-1-valid.pdf", false, false),
        ("pdfua1-rule-7-16-1-bit-10-false.pdf", true, false),
        ("pdfua1-rule-7-16-1-missing-p.pdf", true, true),
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture);
        let report = runner.compare_file(&path, &SafetyLimits::default());
        if reference_parser_discrepancy {
            assert_eq!(
                report.classification,
                ComparisonClassification::ReferenceParserDiscrepancy,
                "{fixture}: {report}"
            );
        } else {
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
}
