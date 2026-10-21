pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.21.8:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-21-8-1-matching.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-8-1-matching.pdf").to_vec(),
            || common::pdfua1_rule_7_21_8_1_fixture("matching"),
            &[],
        ),
        (
            "pdfua1-rule-7-21-8-1-notdef.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-8-1-notdef.pdf").to_vec(),
            || common::pdfua1_rule_7_21_8_1_fixture("fail"),
            &["ISO 14289-1:2014:7.21.8:1"],
        ),
    ],
}
