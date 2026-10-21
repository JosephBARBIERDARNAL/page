pub mod common;

const RULE: &str = "ISO 14289-1:2014:5:4";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-5-4-canonical-prefix.pdf",
            || include_bytes!("fixtures/pdfua1-rule-5-4-canonical-prefix.pdf").to_vec(),
            || common::pdfua1_rule_5_4_fixture("canonical_prefix"),
            &["ISO 14289-1:2014:7.2:33"],
        ),
        (
            "pdfua1-rule-5-4-wrong-prefix.pdf",
            || include_bytes!("fixtures/pdfua1-rule-5-4-wrong-prefix.pdf").to_vec(),
            || common::pdfua1_rule_5_4_fixture("wrong_prefix"),
            &[
                "ISO 14289-1:2014:5:4",
                "ISO 14289-1:2014:7.2:33",
            ],
        ),
    ],
}
