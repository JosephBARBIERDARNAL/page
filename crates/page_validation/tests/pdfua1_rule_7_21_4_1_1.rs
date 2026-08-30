pub mod common;

const RULE: &str = "PDFUA1-FONT-EMBEDDING-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.4.1:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-21-4-1-1-embedded.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-4-1-1-embedded.pdf").to_vec(), || common::pdfua1_rule_7_21_4_1_1_fixture("embedded"), &[], false, false, &[]),
        ("pdfua1-rule-7-21-4-1-1-unembedded.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-4-1-1-unembedded.pdf").to_vec(), || common::pdfua1_rule_7_21_4_1_1_fixture("unembedded"), &["PDFUA1-FONT-EMBEDDING-001"], true, false, &[]),
    ],
}
