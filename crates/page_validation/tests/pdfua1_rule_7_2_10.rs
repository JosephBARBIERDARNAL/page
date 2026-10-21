pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.2:10";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-2-10-allowed.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-10-allowed.pdf").to_vec(),
            || common::pdfua1_rule_7_2_10_fixture("allowed"),
            &[],
        ),
        (
            "pdfua1-rule-7-2-10-invalid.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-10-invalid.pdf").to_vec(),
            || common::pdfua1_rule_7_2_10_fixture("invalid"),
            &["ISO 14289-1:2014:7.2:10"],
        ),
    ],
}
