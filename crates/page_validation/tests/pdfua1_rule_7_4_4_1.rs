pub mod common;

const RULE: &str = "PDFUA1-HEADING-CHILD-COUNT-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.4.4:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-4-4-1-multiple-h.pdf", || include_bytes!("fixtures/pdfua1-rule-7-4-4-1-multiple-h.pdf").to_vec(), || common::pdfua1_rule_7_4_4_1_fixture("multiple_h"), &["PDFUA1-HEADING-CHILD-COUNT-001"], true, false, &[]),
        ("pdfua1-rule-7-4-4-1-single-h.pdf", || include_bytes!("fixtures/pdfua1-rule-7-4-4-1-single-h.pdf").to_vec(), || common::pdfua1_rule_7_4_4_1_fixture("single_h"), &[], false, false, &[]),
    ],
}
