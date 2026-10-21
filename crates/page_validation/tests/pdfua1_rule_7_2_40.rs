pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.2:40";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-2-40-caption-first.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-40-caption-first.pdf").to_vec(),
            || common::pdfua1_rule_7_2_40_fixture("caption_first"),
            &[],
        ),
        (
            "pdfua1-rule-7-2-40-caption-not-first.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-40-caption-not-first.pdf").to_vec(),
            || common::pdfua1_rule_7_2_40_fixture("caption_not_first"),
            &["ISO 14289-1:2014:7.2:40"],
        ),
    ],
}
