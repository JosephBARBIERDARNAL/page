pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.1:7";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-1-7-standard-remapped.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-7-standard-remapped.pdf").to_vec(),
            || common::pdfua1_rule_7_1_7_fixture("standard_remapped"),
            &["ISO 14289-1:2014:7.1:7"],
        ),
        (
            "pdfua1-rule-7-1-7-standard-unmapped.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-7-standard-unmapped.pdf").to_vec(),
            || common::pdfua1_rule_7_1_7_fixture("standard_unmapped"),
            &[],
        ),
    ],
}
