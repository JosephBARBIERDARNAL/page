use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-FORM-FIELD-TU-LANGUAGE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:25";

#[test]
fn pdfua1_rule_7_2_25_requires_language_for_form_field_tu() {
    for (_fixture, case) in [
        ("pdfua1-rule-7-2-25-tu-absent.pdf", "tu_absent"),
        (
            "pdfua1-rule-7-2-25-tu-catalog-language.pdf",
            "tu_present_catalog_language",
        ),
    ] {
        let report = validate_bytes_with_profile(
            &common::pdfua1_rule_7_2_25_fixture(case),
            ValidationProfile::PdfUa1,
            &SafetyLimits::default(),
        );
        assert!(
            !report
                .failures
                .iter()
                .any(|failure| failure.rule_id == RULE)
        );
    }

    let language_missing = validate_bytes_with_profile(
        &common::pdfua1_rule_7_2_25_fixture("tu_present_language_missing"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!language_missing.checks_passed, "{language_missing}");
    assert!(
        language_missing
            .failures
            .iter()
            .any(|failure| failure.rule_id == RULE)
    );

    let tagged_widget_bytes = common::pdfua1_rule_7_18_1_3_fixture("tu");
    let tagged_widget = validate_bytes_with_profile(
        &tagged_widget_bytes,
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(
        !tagged_widget
            .failures
            .iter()
            .any(|failure| failure.rule_id == RULE)
    );

    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let path = env::temp_dir().join(format!(
        "page-pdfua1-rule-7-2-25-tagged-widget-{}.pdf",
        std::process::id()
    ));
    fs::write(&path, tagged_widget_bytes).expect("write tagged-widget differential fixture");
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    let report = runner.compare_file(&path, &SafetyLimits::default());
    let failed = report
        .reference_result
        .as_ref()
        .expect("veraPDF result")
        .failed_rule_ids
        .iter()
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>();
    assert!(!failed.contains(REFERENCE_RULE), "{report}");
    fs::remove_file(path).expect("remove tagged-widget differential fixture");
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.2-25 fixtures"]
fn regenerate_pdfua1_rule_7_2_25_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-2-25-tu-absent.pdf", "tu_absent"),
        (
            "pdfua1-rule-7-2-25-tu-catalog-language.pdf",
            "tu_present_catalog_language",
        ),
        (
            "pdfua1-rule-7-2-25-tu-language-missing.pdf",
            "tu_present_language_missing",
        ),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_2_25_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.2-25 fixture");
    }
}

#[test]
fn pdfua1_rule_7_2_25_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-2-25-tu-absent.pdf", false),
        ("pdfua1-rule-7-2-25-tu-catalog-language.pdf", false),
        ("pdfua1-rule-7-2-25-tu-language-missing.pdf", true),
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture);
        let report = runner.compare_file(&path, &SafetyLimits::default());
        let failed = report
            .reference_result
            .as_ref()
            .unwrap_or_else(|| panic!("{report}"))
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
