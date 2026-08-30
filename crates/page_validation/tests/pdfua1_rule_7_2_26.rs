pub mod common;

const RULE: &str = "PDFUA1-TOCI-PARENT-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:26";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-2-26-contained.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-26-contained.pdf").to_vec(), || common::pdfua1_rule_7_2_26_fixture("contained"), &[], false, false, &[]),
        ("pdfua1-rule-7-2-26-not-contained.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-26-not-contained.pdf").to_vec(), || common::pdfua1_rule_7_2_26_fixture("not_contained"), &["PDFUA1-TOCI-PARENT-001"], true, false, &[]),
    ],
}
