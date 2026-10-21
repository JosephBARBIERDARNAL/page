pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.2:12";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-2-12-allowed.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-12-allowed.pdf").to_vec(),
            || common::pdfua1_rule_7_2_12_fixture("allowed"),
            &[],
        ),
        (
            "pdfua1-rule-7-2-12-invalid.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-12-invalid.pdf").to_vec(),
            || common::pdfua1_rule_7_2_12_fixture("invalid"),
            &["ISO 14289-1:2014:7.2:12"],
        ),
    ],
}
