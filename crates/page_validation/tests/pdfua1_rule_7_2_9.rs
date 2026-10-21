pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.2:9";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-2-9-contained.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-9-contained.pdf").to_vec(),
            || common::pdfua1_rule_7_2_9_fixture("contained"),
            &[],
        ),
        (
            "pdfua1-rule-7-2-9-not-contained.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-9-not-contained.pdf").to_vec(),
            || common::pdfua1_rule_7_2_9_fixture("not_contained"),
            &["ISO 14289-1:2014:7.2:9"],
        ),
    ],
}
