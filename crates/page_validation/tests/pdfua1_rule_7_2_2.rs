pub mod common;

const RULE: &str = "PDFUA1-OUTLINE-LANGUAGE-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-2-2-language-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-2-language-missing.pdf").to_vec(),
            || common::pdfua1_rule_7_2_2_fixture("language_missing"),
            &["PDFUA1-OUTLINE-LANGUAGE-001", "PDFUA1-TEXT-LANGUAGE-001"],
        ),
        (
            "pdfua1-rule-7-2-2-language-present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-2-language-present.pdf").to_vec(),
            || common::pdfua1_rule_7_2_2_fixture("language_present"),
            &[],
        ),
    ],
}
