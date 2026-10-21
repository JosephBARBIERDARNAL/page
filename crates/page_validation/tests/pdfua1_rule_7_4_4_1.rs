pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.4.4:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-4-4-1-multiple-h.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-4-4-1-multiple-h.pdf").to_vec(),
            || common::pdfua1_rule_7_4_4_1_fixture("multiple_h"),
            &["ISO 14289-1:2014:7.4.4:1"],
        ),
        (
            "pdfua1-rule-7-4-4-1-single-h.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-4-4-1-single-h.pdf").to_vec(),
            || common::pdfua1_rule_7_4_4_1_fixture("single_h"),
            &[],
        ),
    ],
}
