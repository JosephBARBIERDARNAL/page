pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.21.3.3:3";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-21-3-3-3-allowed.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-3-3-3-allowed.pdf").to_vec(),
            || common::pdfua1_rule_7_21_3_3_3_fixture("allowed"),
            &[],
        ),
        (
            "pdfua1-rule-7-21-3-3-3-dictionary-unknown.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-3-3-3-dictionary-unknown.pdf").to_vec(),
            || common::pdfua1_rule_7_21_3_3_3_fixture("dictionary_unknown"),
            &["ISO 14289-1:2014:7.21.3.3:3"],
        ),
        (
            "pdfua1-rule-7-21-3-3-3-embedded-unknown.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-3-3-3-embedded-unknown.pdf").to_vec(),
            || common::pdfua1_rule_7_21_3_3_3_fixture("embedded_unknown"),
            &[
                "ISO 14289-1:2014:7.21.3.3:1",
                "ISO 14289-1:2014:7.21.3.3:3",
            ],
        ),
    ],
}
