pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.11:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-11-1-empty-uf.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-11-1-empty-uf.pdf").to_vec(),
            || common::pdfua1_rule_7_11_1_fixture("empty_uf"),
            &["ISO 14289-1:2014:7.11:1"],
        ),
        (
            "pdfua1-rule-7-11-1-valid.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-11-1-valid.pdf").to_vec(),
            || common::pdfua1_rule_7_11_1_fixture("valid"),
            &[],
        ),
    ],
}
