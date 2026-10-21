pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.18.2:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-18-2-1-allowed.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-2-1-allowed.pdf").to_vec(),
            || common::pdfua1_rule_7_18_2_1_fixture("allowed"),
            &[],
        ),
        (
            "pdfua1-rule-7-18-2-1-forbidden.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-2-1-forbidden.pdf").to_vec(),
            || common::pdfua1_rule_7_18_2_1_fixture("forbidden"),
            &["ISO 14289-1:2014:7.18.2:1"],
        ),
    ],
}
