pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.2:22";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-2-22-language-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-22-language-missing.pdf").to_vec(),
            || common::pdfua1_rule_7_2_22_fixture("language_missing"),
            &[
                "ISO 14289-1:2014:7.2:22",
                "ISO 14289-1:2014:7.2:34",
                "ISO 14289-1:2014:7.2:33",
            ],
        ),
        (
            "pdfua1-rule-7-2-22-language-present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-22-language-present.pdf").to_vec(),
            || common::pdfua1_rule_7_2_22_fixture("language_present"),
            &["ISO 14289-1:2014:7.2:34", "ISO 14289-1:2014:7.2:33"],
        ),
    ],
}
