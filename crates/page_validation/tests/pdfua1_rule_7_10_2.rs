pub mod common;

const RULE: &str = "PDFUA1-OPTIONAL-CONTENT-AS-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.10:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-10-2-as-present.pdf", || include_bytes!("fixtures/pdfua1-rule-7-10-2-as-present.pdf").to_vec(), || common::pdfua1_rule_7_10_2_fixture("as_present"), &["PDFUA1-OPTIONAL-CONTENT-AS-001"], true, false, &[]),
        ("pdfua1-rule-7-10-2-valid.pdf", || include_bytes!("fixtures/pdfua1-rule-7-10-2-valid.pdf").to_vec(), || common::pdfua1_rule_7_10_2_fixture("valid"), &[], false, false, &[]),
    ],
}
