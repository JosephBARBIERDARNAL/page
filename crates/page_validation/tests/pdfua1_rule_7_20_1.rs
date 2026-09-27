pub mod common;

const RULE: &str = "PDFUA1-FORM-REFERENCE-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-20-1-allowed.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-20-1-allowed.pdf").to_vec(),
            || common::pdfua1_rule_7_20_1_fixture("allowed"),
            &[],
        ),
        (
            "pdfua1-rule-7-20-1-forbidden.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-20-1-forbidden.pdf").to_vec(),
            || common::pdfua1_rule_7_20_1_fixture("forbidden"),
            &["PDFUA1-FORM-REFERENCE-001"],
        ),
    ],
}
