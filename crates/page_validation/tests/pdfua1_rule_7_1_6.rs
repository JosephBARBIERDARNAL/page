pub mod common;

const RULE: &str = "PDFUA1-STRUCT-TREE-ROLE-MAP-CYCLE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.1:6";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-1-6-acyclic-mapping.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-6-acyclic-mapping.pdf").to_vec(), || common::pdfua1_rule_7_1_6_fixture("acyclic_mapping"), &[], false, false, &[]),
        ("pdfua1-rule-7-1-6-circular-mapping.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-6-circular-mapping.pdf").to_vec(), || common::pdfua1_rule_7_1_6_fixture("circular_mapping"), &["PDFUA1-STRUCT-TREE-ROLE-MAP-CYCLE-001"], true, false, &[]),
    ],
}
