pub mod common;

const RULE: &str = "PDFUA1-TRUETYPE-SYMBOLIC-ENCODING-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.6:3";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-21-6-3-encoding.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-6-3-encoding.pdf").to_vec(), || common::pdfua1_rule_7_21_6_3_fixture("encoding"), &["PDFUA1-TRUETYPE-SYMBOLIC-ENCODING-001"], true, false, &[]),
        ("pdfua1-rule-7-21-6-3-matching.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-6-3-matching.pdf").to_vec(), || common::pdfua1_rule_7_21_6_3_fixture("matching"), &[], false, false, &[]),
    ],
}
