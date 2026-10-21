pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.1:4";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-1-4-false.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-4-false.pdf").to_vec(),
            || common::pdfua1_rule_7_1_4_fixture("false"),
            &["ISO 14289-1:2014:7.2:33"],
        ),
        (
            "pdfua1-rule-7-1-4-true.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-4-true.pdf").to_vec(),
            || common::pdfua1_rule_7_1_4_fixture("true"),
            &["ISO 14289-1:2014:7.1:4", "ISO 14289-1:2014:7.2:33"],
        ),
    ],
}
