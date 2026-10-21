pub mod common;

const RULE: &str = "ISO 14289-1:2014:5:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-5-1-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-5-1-missing.pdf").to_vec(),
            || common::pdfua1_rule_5_1_fixture("missing"),
            &[
                "ISO 14289-1:2014:5:2",
                "ISO 14289-1:2014:5:1",
                "ISO 14289-1:2014:7.2:33",
            ],
        ),
        (
            "pdfua1-rule-5-1-present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-5-1-present.pdf").to_vec(),
            || common::pdfua1_rule_5_1_fixture("present"),
            &["ISO 14289-1:2014:7.2:33"],
        ),
    ],
}
