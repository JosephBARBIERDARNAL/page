pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.18.1:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-18-1-1-invalid.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-1-1-invalid.pdf").to_vec(),
            || common::pdfua1_rule_7_18_1_1_fixture("invalid"),
            &["ISO 14289-1:2014:7.18.1:1"],
        ),
        (
            "pdfua1-rule-7-18-1-1-valid.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-1-1-valid.pdf").to_vec(),
            || common::pdfua1_rule_7_18_1_1_fixture("valid"),
            &[],
        ),
    ],
}
