pub mod common;

const RULE: &str = "ISO 14289-1:2014:6.2:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-6-2-marked.pdf",
            || include_bytes!("fixtures/pdfua1-rule-6-2-marked.pdf").to_vec(),
            || common::pdfua1_rule_6_2_fixture("marked"),
            &["ISO 14289-1:2014:7.2:33"],
        ),
        (
            "pdfua1-rule-6-2-unmarked.pdf",
            || include_bytes!("fixtures/pdfua1-rule-6-2-unmarked.pdf").to_vec(),
            || common::pdfua1_rule_6_2_fixture("unmarked"),
            &[
                "ISO 14289-1:2014:6.2:1",
                "ISO 14289-1:2014:7.2:33",
            ],
        ),
    ],
}
