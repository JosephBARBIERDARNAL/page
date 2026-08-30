pub mod common;

const RULE: &str = "PDFUA1-SPAN-ACTUAL-TEXT-LANGUAGE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:30";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-2-30-catalog_language_present.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-30-catalog_language_present.pdf").to_vec(), || common::pdfua1_rule_7_2_30_fixture("catalog_language_present"), &[], false, false, &[]),
        ("pdfua1-rule-7-2-30-inherited_language_present.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-30-inherited_language_present.pdf").to_vec(), || common::pdfua1_rule_7_2_30_fixture("inherited_language_present"), &[], false, false, &[]),
        ("pdfua1-rule-7-2-30-language_missing.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-30-language_missing.pdf").to_vec(), || common::pdfua1_rule_7_2_30_fixture("language_missing"), &["PDFUA1-SPAN-ACTUAL-TEXT-LANGUAGE-001", "PDFUA1-TEXT-LANGUAGE-001"], true, false, &[]),
        ("pdfua1-rule-7-2-30-property_language_present.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-30-property_language_present.pdf").to_vec(), || common::pdfua1_rule_7_2_30_fixture("property_language_present"), &[], false, false, &[]),
    ],
}
