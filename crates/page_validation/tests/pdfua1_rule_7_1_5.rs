pub mod common;

const RULE: &str = "PDFUA1-STRUCT-TREE-ROLE-MAP-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.1:5";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-1-5-indirect-mapping.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-5-indirect-mapping.pdf").to_vec(), || common::pdfua1_rule_7_1_5_fixture("indirect_mapping"), &[], false, false, &[]),
        ("pdfua1-rule-7-1-5-unmapped.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-5-unmapped.pdf").to_vec(), || common::pdfua1_rule_7_1_5_fixture("unmapped"), &["PDFUA1-STRUCT-TREE-ROLE-MAP-001"], true, false, &[]),
    ],
}
