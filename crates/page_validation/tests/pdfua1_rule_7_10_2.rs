pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.10:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-10-2-as-present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-10-2-as-present.pdf").to_vec(),
            || common::pdfua1_rule_7_10_2_fixture("as_present"),
            &["ISO 14289-1:2014:7.10:2"],
        ),
        (
            "pdfua1-rule-7-10-2-valid.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-10-2-valid.pdf").to_vec(),
            || common::pdfua1_rule_7_10_2_fixture("valid"),
            &[],
        ),
    ],
}
