pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.1:11";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-1-11-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-11-missing.pdf").to_vec(),
            || common::pdfua1_rule_7_1_11_fixture("missing"),
            &["ISO 14289-1:2014:7.1:11", "ISO 14289-1:2014:7.2:33"],
        ),
        (
            "pdfua1-rule-7-1-11-present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-11-present.pdf").to_vec(),
            || common::pdfua1_rule_7_1_11_fixture("present"),
            &["ISO 14289-1:2014:7.2:33"],
        ),
    ],
}
