pub mod common;

const RULE: &str = "PDFUA1-CID-SUBSET-CIDSET-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.4.2:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-21-4-2-2-complete.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-4-2-2-complete.pdf").to_vec(), || common::pdfua1_rule_7_21_4_2_2_fixture("complete"), &[], false, false, &[]),
        ("pdfua1-rule-7-21-4-2-2-incomplete.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-4-2-2-incomplete.pdf").to_vec(), || common::pdfua1_rule_7_21_4_2_2_fixture("incomplete"), &["PDFUA1-CID-SUBSET-CIDSET-001"], true, false, &[]),
    ],
}
