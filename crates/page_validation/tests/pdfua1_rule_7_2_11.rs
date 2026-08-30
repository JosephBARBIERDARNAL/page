pub mod common;

const RULE: &str = "PDFUA1-TABLE-THEAD-COUNT-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:11";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-2-11-allowed.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-11-allowed.pdf").to_vec(), || common::pdfua1_rule_7_2_11_fixture("allowed"), &[], false, false, &[]),
        ("pdfua1-rule-7-2-11-invalid.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-11-invalid.pdf").to_vec(), || common::pdfua1_rule_7_2_11_fixture("invalid"), &["PDFUA1-TABLE-THEAD-COUNT-001"], true, false, &[]),
    ],
}
