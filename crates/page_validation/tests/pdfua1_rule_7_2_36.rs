pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.2:36";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-2-36-allowed.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-36-allowed.pdf").to_vec(),
            || common::pdfua1_rule_7_2_36_fixture("allowed"),
            &[],
        ),
        (
            "pdfua1-rule-7-2-36-invalid.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-36-invalid.pdf").to_vec(),
            || common::pdfua1_rule_7_2_36_fixture("invalid"),
            &["ISO 14289-1:2014:7.2:36"],
        ),
    ],
}
