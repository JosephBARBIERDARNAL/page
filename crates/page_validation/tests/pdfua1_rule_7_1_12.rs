pub mod common;

const RULE: &str = "PDFUA1-STRUCT-ELEMENT-PARENT-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.1:12";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-1-12-missing.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-12-missing.pdf").to_vec(), || common::pdfua1_rule_7_1_12_fixture("missing"), &["PDFUA1-STRUCT-ELEMENT-PARENT-001"], true, false, &[]),
        ("pdfua1-rule-7-1-12-present.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-12-present.pdf").to_vec(), || common::pdfua1_rule_7_1_12_fixture("present"), &[], false, false, &[]),
    ],
}
