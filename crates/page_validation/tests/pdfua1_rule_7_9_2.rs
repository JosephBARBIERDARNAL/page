pub mod common;

const RULE: &str = "PDFUA1-NOTE-ID-UNIQUE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.9:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-9-2-duplicate.pdf", || include_bytes!("fixtures/pdfua1-rule-7-9-2-duplicate.pdf").to_vec(), || common::pdfua1_rule_7_9_2_fixture("duplicate"), &["PDFUA1-NOTE-ID-UNIQUE-001"], true, false, &[]),
        ("pdfua1-rule-7-9-2-unique.pdf", || include_bytes!("fixtures/pdfua1-rule-7-9-2-unique.pdf").to_vec(), || common::pdfua1_rule_7_9_2_fixture("unique"), &[], false, false, &[]),
    ],
}
