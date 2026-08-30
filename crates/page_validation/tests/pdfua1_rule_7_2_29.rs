pub mod common;

const RULE: &str = "PDFUA1-LANGUAGE-TAG-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:29";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-2-29-catalog_invalid.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-29-catalog_invalid.pdf").to_vec(), || common::pdfua1_rule_7_2_29_fixture("catalog_invalid"), &["PDFUA1-LANGUAGE-TAG-001"], true, false, &[]),
        ("pdfua1-rule-7-2-29-catalog_valid.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-29-catalog_valid.pdf").to_vec(), || common::pdfua1_rule_7_2_29_fixture("catalog_valid"), &[], false, false, &[]),
        ("pdfua1-rule-7-2-29-property_invalid.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-29-property_invalid.pdf").to_vec(), || common::pdfua1_rule_7_2_29_fixture("property_invalid"), &["PDFUA1-LANGUAGE-TAG-001"], true, false, &[]),
        ("pdfua1-rule-7-2-29-property_valid.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-29-property_valid.pdf").to_vec(), || common::pdfua1_rule_7_2_29_fixture("property_valid"), &[], false, false, &[]),
        ("pdfua1-rule-7-2-29-structure_invalid.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-29-structure_invalid.pdf").to_vec(), || common::pdfua1_rule_7_2_29_fixture("structure_invalid"), &["PDFUA1-LANGUAGE-TAG-001"], true, false, &[]),
        ("pdfua1-rule-7-2-29-structure_valid.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-29-structure_valid.pdf").to_vec(), || common::pdfua1_rule_7_2_29_fixture("structure_valid"), &[], false, false, &[]),
    ],
}
