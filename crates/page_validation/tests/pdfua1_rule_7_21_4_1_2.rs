pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.21.4.1:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-21-4-1-2-invisible.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-4-1-2-invisible.pdf").to_vec(),
            || common::pdfua1_rule_7_21_4_1_2_fixture("invisible"),
            &["ISO 14289-1:2014:7.1:3"],
        ),
        (
            "pdfua1-rule-7-21-4-1-2-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-4-1-2-missing.pdf").to_vec(),
            || common::pdfua1_rule_7_21_4_1_2_fixture("missing"),
            &["ISO 14289-1:2014:7.1:3", "ISO 14289-1:2014:7.21.4.1:2"],
        ),
        (
            "pdfua1-rule-7-21-4-1-2-present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-4-1-2-present.pdf").to_vec(),
            || common::pdfua1_rule_7_21_4_1_2_fixture("present"),
            &["ISO 14289-1:2014:7.1:3"],
        ),
    ],
}
