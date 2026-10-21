pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.1:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-1-2-inside-artifact.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-2-inside-artifact.pdf").to_vec(),
            || common::pdfua1_rule_7_1_2_fixture("inside_artifact"),
            &["ISO 14289-1:2014:7.1:2"],
        ),
        (
            "pdfua1-rule-7-1-2-outside-artifact.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-2-outside-artifact.pdf").to_vec(),
            || common::pdfua1_rule_7_1_2_fixture("outside_artifact"),
            &[],
        ),
    ],
}
