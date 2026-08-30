pub mod common;

const RULE: &str = "PDFUA1-METADATA-LANGUAGE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:33";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-2-33-catalog_language.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-33-catalog_language.pdf").to_vec(), || common::pdfua1_rule_7_2_33_fixture("catalog_language"), &[], false, false, &[]),
        ("pdfua1-rule-7-2-33-missing_x_default.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-33-missing_x_default.pdf").to_vec(), || common::pdfua1_rule_7_2_33_fixture("missing_x_default"), &["PDFUA1-TEXT-LANGUAGE-001"], false, false, &[]),
        ("pdfua1-rule-7-2-33-multiple_items.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-33-multiple_items.pdf").to_vec(), || common::pdfua1_rule_7_2_33_fixture("multiple_items"), &["PDFUA1-TEXT-LANGUAGE-001"], false, false, &[]),
        ("pdfua1-rule-7-2-33-x_default.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-33-x_default.pdf").to_vec(), || common::pdfua1_rule_7_2_33_fixture("x_default"), &["PDFUA1-TEXT-LANGUAGE-001"], false, false, &[]),
    ],
}
