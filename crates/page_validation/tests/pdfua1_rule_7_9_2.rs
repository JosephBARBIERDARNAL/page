pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.9:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-9-2-duplicate.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-9-2-duplicate.pdf").to_vec(),
            || common::pdfua1_rule_7_9_2_fixture("duplicate"),
            &["ISO 14289-1:2014:7.9:2"],
        ),
        (
            "pdfua1-rule-7-9-2-unique.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-9-2-unique.pdf").to_vec(),
            || common::pdfua1_rule_7_9_2_fixture("unique"),
            &[],
        ),
    ],
}
