pub mod common;

const RULE: &str = "PDFUA1-STRUCT-TREE-ROLE-MAP-STANDARD-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.1:7";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-1-7-standard-remapped.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-7-standard-remapped.pdf").to_vec(), || common::pdfua1_rule_7_1_7_fixture("standard_remapped"), &["PDFUA1-STRUCT-TREE-ROLE-MAP-STANDARD-001"], true, false, &[]),
        ("pdfua1-rule-7-1-7-standard-unmapped.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-7-standard-unmapped.pdf").to_vec(), || common::pdfua1_rule_7_1_7_fixture("standard_unmapped"), &[], false, false, &[]),
    ],
}
