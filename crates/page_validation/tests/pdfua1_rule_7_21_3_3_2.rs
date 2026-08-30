pub mod common;

const RULE: &str = "PDFUA1-CMAP-WMODE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.3.3:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-21-3-3-2-matching.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-3-3-2-matching.pdf").to_vec(), || common::pdfua1_rule_7_21_3_3_2_fixture("matching"), &[], false, false, &[]),
        ("pdfua1-rule-7-21-3-3-2-mismatched.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-3-3-2-mismatched.pdf").to_vec(), || common::pdfua1_rule_7_21_3_3_2_fixture("mismatched"), &["PDFUA1-CMAP-WMODE-001"], true, false, &[]),
    ],
}
