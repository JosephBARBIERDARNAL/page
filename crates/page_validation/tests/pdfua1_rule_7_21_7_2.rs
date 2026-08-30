pub mod common;

const RULE: &str = "PDFUA1-FONT-UNICODE-VALUE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.7:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-21-7-2-feff.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-7-2-feff.pdf").to_vec(), || common::pdfua1_rule_7_21_7_2_fixture("feff"), &["PDFUA1-FONT-UNICODE-VALUE-001"], true, false, &[]),
        ("pdfua1-rule-7-21-7-2-fffe.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-7-2-fffe.pdf").to_vec(), || common::pdfua1_rule_7_21_7_2_fixture("fffe"), &["PDFUA1-FONT-UNICODE-VALUE-001"], true, false, &[]),
        ("pdfua1-rule-7-21-7-2-matching.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-7-2-matching.pdf").to_vec(), || common::pdfua1_rule_7_21_7_2_fixture("matching"), &[], false, false, &[]),
        ("pdfua1-rule-7-21-7-2-zero.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-7-2-zero.pdf").to_vec(), || common::pdfua1_rule_7_21_7_2_fixture("zero"), &["PDFUA1-FONT-UNICODE-VALUE-001"], true, false, &[]),
    ],
}
