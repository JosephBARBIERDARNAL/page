use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_pdf_bytes};

pub mod common;

const RULE: &str = "PDFUA1-OPTIONAL-CONTENT-AS-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.10:2";

#[test]
fn pdfua1_rule_7_10_2_rejects_as_in_optional_content_configurations() {
    let valid = validate_pdf_bytes(
        include_bytes!("fixtures/pdfua1-rule-7-10-2-valid.pdf"),
        Some(ValidationProfile::PdfUa1),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");
    assert!(valid.is_compliant, "{valid}");
    assert!(valid.failures.is_empty(), "{valid}");

    let as_present = validate_pdf_bytes(
        include_bytes!("fixtures/pdfua1-rule-7-10-2-as-present.pdf"),
        Some(ValidationProfile::PdfUa1),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");
    assert!(!as_present.is_compliant, "{as_present}");
    assert_eq!(as_present.checks.failed, 1, "{as_present}");
    assert_eq!(as_present.failures.len(), 1, "{as_present}");
    assert_eq!(as_present.failures[0].rule_id, RULE, "{as_present}");
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.10-2 fixtures"]
fn regenerate_pdfua1_rule_7_10_2_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-10-2-valid.pdf", "valid"),
        ("pdfua1-rule-7-10-2-as-present.pdf", "as_present"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_10_2_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.10-2 fixture");
    }
}

#[test]
fn pdfua1_rule_7_10_2_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-10-2-valid.pdf", false),
        ("pdfua1-rule-7-10-2-as-present.pdf", true),
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
