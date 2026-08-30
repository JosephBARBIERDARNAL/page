pub mod common;

const RULE: &str = "PDFUA1-METADATA-TITLE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.1:9";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-1-9-missing.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-9-missing.pdf").to_vec(), || common::pdfua1_rule_7_1_9_fixture("missing"), &["PDFUA1-METADATA-TITLE-001"], true, false, &[]),
        ("pdfua1-rule-7-1-9-present.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-9-present.pdf").to_vec(), || common::pdfua1_rule_7_1_9_fixture("present"), &[], false, false, &[]),
    ],
}
