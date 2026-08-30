pub mod common;

const RULE: &str = "PDFUA1-SUSPECTS-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.1:4";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-1-4-false.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-4-false.pdf").to_vec(), || common::pdfua1_rule_7_1_4_fixture("false"), &[], false, false, &[]),
        ("pdfua1-rule-7-1-4-true.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-4-true.pdf").to_vec(), || common::pdfua1_rule_7_1_4_fixture("true"), &["PDFUA1-SUSPECTS-001"], true, false, &[]),
    ],
}
