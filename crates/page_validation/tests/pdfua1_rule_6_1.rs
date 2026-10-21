pub mod common;

const RULE: &str = "ISO 14289-1:2014:6.1:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-6-1-invalid-header.pdf",
            || include_bytes!("fixtures/pdfua1-rule-6-1-invalid-header.pdf").to_vec(),
            || common::pdfua1_rule_6_1_fixture("invalid_header"),
            &["ISO 14289-1:2014:6.1:1", "ISO 14289-1:2014:7.2:33"],
        ),
        (
            "pdfua1-rule-6-1-valid-header.pdf",
            || include_bytes!("fixtures/pdfua1-rule-6-1-valid-header.pdf").to_vec(),
            || common::pdfua1_rule_6_1_fixture("valid_header"),
            &["ISO 14289-1:2014:7.2:33"],
        ),
    ],
}
