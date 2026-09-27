pub mod common;

const RULE: &str = "PDFUA1-FORM-FIELD-TU-ALT-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-18-1-3-alt.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-1-3-alt.pdf").to_vec(),
            || common::pdfua1_rule_7_18_1_3_fixture("alt"),
            &[],
        ),
        (
            "pdfua1-rule-7-18-1-3-empty-tu.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-1-3-empty-tu.pdf").to_vec(),
            || common::pdfua1_rule_7_18_1_3_fixture("empty_tu"),
            &["PDFUA1-FORM-FIELD-TU-ALT-001"],
        ),
        (
            "pdfua1-rule-7-18-1-3-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-1-3-missing.pdf").to_vec(),
            || common::pdfua1_rule_7_18_1_3_fixture("missing"),
            &["PDFUA1-FORM-FIELD-TU-ALT-001"],
        ),
        (
            "pdfua1-rule-7-18-1-3-tu.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-1-3-tu.pdf").to_vec(),
            || common::pdfua1_rule_7_18_1_3_fixture("tu"),
            &[],
        ),
    ],
}
