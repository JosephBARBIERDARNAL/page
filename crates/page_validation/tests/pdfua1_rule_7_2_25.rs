pub mod common;

const RULE: &str = "PDFUA1-FORM-FIELD-TU-LANGUAGE-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-2-25-tu-absent.pdf",
            || common::pdfua1_rule_7_2_25_fixture("tu_absent"),
            || common::pdfua1_rule_7_2_25_fixture("tu_absent"),
            &["PDFUA1-TEXT-LANGUAGE-001"],
        ),
        (
            "pdfua1-rule-7-2-25-tu-catalog-language.pdf",
            || common::pdfua1_rule_7_2_25_fixture("tu_present_catalog_language"),
            || common::pdfua1_rule_7_2_25_fixture("tu_present_catalog_language"),
            &[],
        ),
        (
            "pdfua1-rule-7-2-25-tu-language-missing.pdf",
            || common::pdfua1_rule_7_2_25_fixture("tu_present_language_missing"),
            || common::pdfua1_rule_7_2_25_fixture("tu_present_language_missing"),
            &[RULE, "PDFUA1-TEXT-LANGUAGE-001"],
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
