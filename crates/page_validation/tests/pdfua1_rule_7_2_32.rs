pub mod common;

const RULE: &str = "PDFUA1-SPAN-E-TEXT-LANGUAGE-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-2-32-catalog_language_present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-32-catalog_language_present.pdf").to_vec(),
            || common::pdfua1_rule_7_2_32_fixture("catalog_language_present"),
            &[],
        ),
        (
            "pdfua1-rule-7-2-32-inherited_language_present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-32-inherited_language_present.pdf").to_vec(),
            || common::pdfua1_rule_7_2_32_fixture("inherited_language_present"),
            &[],
        ),
        (
            "pdfua1-rule-7-2-32-language_missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-32-language_missing.pdf").to_vec(),
            || common::pdfua1_rule_7_2_32_fixture("language_missing"),
            &["PDFUA1-SPAN-E-TEXT-LANGUAGE-001", "PDFUA1-TEXT-LANGUAGE-001"],
        ),
        (
            "pdfua1-rule-7-2-32-property_language_present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-32-property_language_present.pdf").to_vec(),
            || common::pdfua1_rule_7_2_32_fixture("property_language_present"),
            &[],
        ),
    ],
}
