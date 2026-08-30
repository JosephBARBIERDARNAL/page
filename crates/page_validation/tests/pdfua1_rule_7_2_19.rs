pub mod common;

const RULE: &str = "PDFUA1-L-KIDS-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:19";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-2-19-allowed.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-19-allowed.pdf").to_vec(), || common::pdfua1_rule_7_2_19_fixture("allowed"), &[], false, false, &[]),
        ("pdfua1-rule-7-2-19-invalid.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-19-invalid.pdf").to_vec(), || common::pdfua1_rule_7_2_19_fixture("invalid"), &["PDFUA1-L-KIDS-001"], true, false, &[]),
    ],
}
