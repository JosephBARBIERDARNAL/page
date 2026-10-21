pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.1:9";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-1-9-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-9-missing.pdf").to_vec(),
            || common::pdfua1_rule_7_1_9_fixture("missing"),
            &["ISO 14289-1:2014:7.1:9"],
        ),
        (
            "pdfua1-rule-7-1-9-present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-9-present.pdf").to_vec(),
            || common::pdfua1_rule_7_1_9_fixture("present"),
            &["ISO 14289-1:2014:7.2:33"],
        ),
    ],
}
