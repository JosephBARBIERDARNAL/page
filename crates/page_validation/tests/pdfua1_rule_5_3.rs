pub mod common;

const RULE: &str = "ISO 14289-1:2014:5:3";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-5-3-canonical-prefix.pdf",
            || include_bytes!("fixtures/pdfua1-rule-5-3-canonical-prefix.pdf").to_vec(),
            || common::pdfua1_rule_5_3_fixture("canonical_prefix"),
            &["ISO 14289-1:2014:7.2:33"],
        ),
        (
            "pdfua1-rule-5-3-wrong-prefix.pdf",
            || include_bytes!("fixtures/pdfua1-rule-5-3-wrong-prefix.pdf").to_vec(),
            || common::pdfua1_rule_5_3_fixture("wrong_prefix"),
            &[
                "ISO 14289-1:2014:5:3",
                "ISO 14289-1:2014:7.2:33",
            ],
        ),
    ],
}
