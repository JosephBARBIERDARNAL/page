pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.9:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-9-1-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-9-1-missing.pdf").to_vec(),
            || common::pdfua1_rule_7_9_1_fixture("missing"),
            &["ISO 14289-1:2014:7.9:1"],
        ),
        (
            "pdfua1-rule-7-9-1-present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-9-1-present.pdf").to_vec(),
            || common::pdfua1_rule_7_9_1_fixture("present"),
            &[],
        ),
    ],
}
