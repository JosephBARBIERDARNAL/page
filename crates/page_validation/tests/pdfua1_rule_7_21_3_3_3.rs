pub mod common;

const RULE: &str = "PDFUA1-CMAP-REFERENCE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.3.3:3";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-21-3-3-3-allowed.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-3-3-3-allowed.pdf").to_vec(), || common::pdfua1_rule_7_21_3_3_3_fixture("allowed"), &[], false, false, &[]),
        ("pdfua1-rule-7-21-3-3-3-dictionary-unknown.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-3-3-3-dictionary-unknown.pdf").to_vec(), || common::pdfua1_rule_7_21_3_3_3_fixture("dictionary_unknown"), &["PDFUA1-CMAP-REFERENCE-001"], true, false, &[]),
        ("pdfua1-rule-7-21-3-3-3-embedded-unknown.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-3-3-3-embedded-unknown.pdf").to_vec(), || common::pdfua1_rule_7_21_3_3_3_fixture("embedded_unknown"), &["PDFUA1-CMAP-REFERENCE-001"], true, false, &[]),
    ],
}
