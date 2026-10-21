pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.2:31";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-2-31-catalog_language_present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-31-catalog_language_present.pdf").to_vec(),
            || common::pdfua1_rule_7_2_31_fixture("catalog_language_present"),
            &[],
        ),
        (
            "pdfua1-rule-7-2-31-inherited_language_present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-31-inherited_language_present.pdf").to_vec(),
            || common::pdfua1_rule_7_2_31_fixture("inherited_language_present"),
            &["ISO 14289-1:2014:7.2:33"],
        ),
        (
            "pdfua1-rule-7-2-31-language_missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-31-language_missing.pdf").to_vec(),
            || common::pdfua1_rule_7_2_31_fixture("language_missing"),
            &[
                "ISO 14289-1:2014:7.2:31",
                "ISO 14289-1:2014:7.2:34",
                "ISO 14289-1:2014:7.2:33",
            ],
        ),
        (
            "pdfua1-rule-7-2-31-property_language_present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-31-property_language_present.pdf").to_vec(),
            || common::pdfua1_rule_7_2_31_fixture("property_language_present"),
            &["ISO 14289-1:2014:7.2:33"],
        ),
    ],
}
