pub mod common;

const RULE: &str = "PDFUA1-FORM-CHILDREN-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.18.4:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-18-4-2-allowed.pdf", || include_bytes!("fixtures/pdfua1-rule-7-18-4-2-allowed.pdf").to_vec(), || common::pdfua1_rule_7_18_4_2_fixture("allowed"), &[], false, false, &[]),
        ("pdfua1-rule-7-18-4-2-invalid.pdf", || include_bytes!("fixtures/pdfua1-rule-7-18-4-2-invalid.pdf").to_vec(), || common::pdfua1_rule_7_18_4_2_fixture("invalid"), &["PDFUA1-FORM-CHILDREN-001"], true, false, &[]),
        ("pdfua1-rule-7-18-4-2-role-attribute.pdf", || include_bytes!("fixtures/pdfua1-rule-7-18-4-2-role-attribute.pdf").to_vec(), || common::pdfua1_rule_7_18_4_2_fixture("role_attribute"), &[], false, false, &[]),
    ],
}
