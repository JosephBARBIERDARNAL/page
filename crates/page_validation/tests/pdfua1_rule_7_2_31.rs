use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_pdf_bytes};

pub mod common;

const RULE: &str = "PDFUA1-SPAN-ALT-TEXT-LANGUAGE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:31";

#[test]
fn pdfua1_rule_7_2_31_requires_language_for_span_alt_text() {
    for case in [
        "property_language_present",
        "inherited_language_present",
        "catalog_language_present",
    ] {
        let report = validate_pdf_bytes(
            &common::pdfua1_rule_7_2_31_fixture(case),
            Some(ValidationProfile::PdfUa1),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert!(
            !report
                .failures
                .iter()
                .any(|failure| failure.rule_id == RULE)
        );
    }

    let report = validate_pdf_bytes(
        &common::pdfua1_rule_7_2_31_fixture("language_missing"),
        Some(ValidationProfile::PdfUa1),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");
    assert!(!report.is_compliant, "{report}");
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.rule_id == RULE)
    );
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.2-31 fixtures"]
fn regenerate_pdfua1_rule_7_2_31_fixtures() {
    for case in [
        "property_language_present",
        "inherited_language_present",
        "catalog_language_present",
        "language_missing",
    ] {
        fs::write(
            Path::new("tests/fixtures").join(format!("pdfua1-rule-7-2-31-{case}.pdf")),
            common::pdfua1_rule_7_2_31_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.2-31 fixture");
    }
}

#[test]
fn pdfua1_rule_7_2_31_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (case, should_fail) in [
        ("property_language_present", false),
        ("inherited_language_present", false),
        ("catalog_language_present", false),
        ("language_missing", true),
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(format!("pdfua1-rule-7-2-31-{case}.pdf"));
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
            "{case}: {report}"
        );
    }
}
