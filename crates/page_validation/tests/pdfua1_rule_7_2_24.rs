pub mod common;

const RULE: &str = "PDFUA1-ANNOTATION-CONTENTS-LANGUAGE-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-2-24-annotation-language-present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-24-annotation-language-present.pdf").to_vec(),
            || common::pdfua1_rule_7_2_24_fixture("annotation_language_present"),
            &["PDFUA1-TEXT-LANGUAGE-001"],
        ),
        (
            "pdfua1-rule-7-2-24-catalog-language-present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-24-catalog-language-present.pdf").to_vec(),
            || common::pdfua1_rule_7_2_24_fixture("catalog_language_present"),
            &[],
        ),
        (
            "pdfua1-rule-7-2-24-language-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-24-language-missing.pdf").to_vec(),
            || common::pdfua1_rule_7_2_24_fixture("language_missing"),
            &["PDFUA1-ANNOTATION-CONTENTS-LANGUAGE-001", "PDFUA1-TEXT-LANGUAGE-001"],
        ),
    ],
}
