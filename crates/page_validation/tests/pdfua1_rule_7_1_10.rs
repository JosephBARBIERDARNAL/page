pub mod common;

const RULE: &str = "PDFUA1-VIEWER-PREFERENCES-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.1:10";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-1-10-false.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-10-false.pdf").to_vec(), || common::pdfua1_rule_7_1_10_fixture("false"), &["PDFUA1-VIEWER-PREFERENCES-001"], true, false, &[]),
        ("pdfua1-rule-7-1-10-missing.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-10-missing.pdf").to_vec(), || common::pdfua1_rule_7_1_10_fixture("missing"), &["PDFUA1-VIEWER-PREFERENCES-001"], true, false, &[]),
        ("pdfua1-rule-7-1-10-present.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-10-present.pdf").to_vec(), || common::pdfua1_rule_7_1_10_fixture("present"), &[], false, false, &[]),
    ],
}
