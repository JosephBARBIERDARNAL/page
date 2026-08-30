use std::env;
use std::fs;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};

pub mod common;

const RULE: &str = "PDFUA1-FORM-FIELD-TU-LANGUAGE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:25";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        (
            "pdfua1-rule-7-2-25-tu-absent.pdf",
            || common::pdfua1_rule_7_2_25_fixture("tu_absent"),
            || common::pdfua1_rule_7_2_25_fixture("tu_absent"),
            &["PDFUA1-TEXT-LANGUAGE-001"],
            false,
            false,
            &[],
        ),
        (
            "pdfua1-rule-7-2-25-tu-catalog-language.pdf",
            || common::pdfua1_rule_7_2_25_fixture("tu_present_catalog_language"),
            || common::pdfua1_rule_7_2_25_fixture("tu_present_catalog_language"),
            &[],
            false,
            false,
            &[],
        ),
        (
            "pdfua1-rule-7-2-25-tu-language-missing.pdf",
            || common::pdfua1_rule_7_2_25_fixture("tu_present_language_missing"),
            || common::pdfua1_rule_7_2_25_fixture("tu_present_language_missing"),
            &[RULE, "PDFUA1-TEXT-LANGUAGE-001"],
            true,
            false,
            &[],
        ),
    ],
}

#[test]
fn tagged_widget_case_remains_inapplicable() {
    let bytes = common::pdfua1_rule_7_18_1_3_fixture("tu");
    let report = page_validation::validate_pdf_bytes(
        &bytes,
        Some(page_validation::ValidationProfile::PdfUa1),
        &page_validation::SafetyLimits::default(),
    )
    .expect("explicit PDF/UA-1 profile validation");
    assert!(
        !report
            .failures
            .iter()
            .any(|failure| failure.rule_id == RULE),
        "{report}"
    );
}

#[test]
fn tagged_widget_case_matches_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let path = env::temp_dir().join(format!(
        "page-pdfua1-rule-7-2-25-tagged-widget-{}.pdf",
        std::process::id()
    ));
    fs::write(&path, common::pdfua1_rule_7_18_1_3_fixture("tu"))
        .expect("write tagged-widget differential fixture");
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    let report = runner.compare_file(&path, &page_validation::SafetyLimits::default());
    let reference = report.reference_result.as_ref().expect("veraPDF result");
    assert!(
        !reference
            .failed_rule_ids
            .iter()
            .any(|rule| rule.to_string() == REFERENCE_RULE),
        "{report}"
    );
    fs::remove_file(path).expect("remove tagged-widget differential fixture");
}
