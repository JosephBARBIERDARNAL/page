pub mod common;

const RULE: &str = "PDFUA1-E-TEXT-LANGUAGE-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-2-23-language-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-23-language-missing.pdf").to_vec(),
            || common::pdfua1_rule_7_2_23_fixture("language_missing"),
            &["PDFUA1-E-TEXT-LANGUAGE-001", "PDFUA1-TEXT-LANGUAGE-001"],
        ),
        (
            "pdfua1-rule-7-2-23-language-present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-23-language-present.pdf").to_vec(),
            || common::pdfua1_rule_7_2_23_fixture("language_present"),
            &["PDFUA1-TEXT-LANGUAGE-001"],
        ),
    ],
}
