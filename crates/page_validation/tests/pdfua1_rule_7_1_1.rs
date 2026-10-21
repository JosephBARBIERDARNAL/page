pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.1:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-1-1-inside-tagged-content.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-1-inside-tagged-content.pdf").to_vec(),
            || common::pdfua1_rule_7_1_1_fixture("inside_tagged_content"),
            &["ISO 14289-1:2014:7.1:1"],
        ),
        (
            "pdfua1-rule-7-1-1-outside-tagged-content.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-1-outside-tagged-content.pdf").to_vec(),
            || common::pdfua1_rule_7_1_1_fixture("outside_tagged_content"),
            &[],
        ),
    ],
}
