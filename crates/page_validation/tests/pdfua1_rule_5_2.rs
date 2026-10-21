pub mod common;

const RULE: &str = "ISO 14289-1:2014:5:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-5-2-present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-5-2-present.pdf").to_vec(),
            || common::pdfua1_rule_5_2_fixture("present"),
            &["ISO 14289-1:2014:7.2:33"],
        ),
        (
            "pdfua1-rule-5-2-wrong-part.pdf",
            || include_bytes!("fixtures/pdfua1-rule-5-2-wrong-part.pdf").to_vec(),
            || common::pdfua1_rule_5_2_fixture("wrong_part"),
            &["ISO 14289-1:2014:5:2", "ISO 14289-1:2014:7.2:33"],
        ),
    ],
}
