pub mod common;

const RULE: &str = "PDFUA1-TAGGED-DOCUMENT-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:6.2:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-6-2-marked.pdf", || include_bytes!("fixtures/pdfua1-rule-6-2-marked.pdf").to_vec(), || common::pdfua1_rule_6_2_fixture("marked"), &[], false, false, &[]),
        ("pdfua1-rule-6-2-unmarked.pdf", || include_bytes!("fixtures/pdfua1-rule-6-2-unmarked.pdf").to_vec(), || common::pdfua1_rule_6_2_fixture("unmarked"), &["PDFUA1-TAGGED-DOCUMENT-001"], true, false, &[]),
    ],
}
