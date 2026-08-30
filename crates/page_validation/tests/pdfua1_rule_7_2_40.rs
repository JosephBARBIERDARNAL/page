pub mod common;

const RULE: &str = "PDFUA1-L-CAPTION-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:40";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-2-40-caption-first.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-40-caption-first.pdf").to_vec(), || common::pdfua1_rule_7_2_40_fixture("caption_first"), &[], false, false, &[]),
        ("pdfua1-rule-7-2-40-caption-not-first.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-40-caption-not-first.pdf").to_vec(), || common::pdfua1_rule_7_2_40_fixture("caption_not_first"), &["PDFUA1-L-CAPTION-001"], true, false, &[]),
    ],
}
