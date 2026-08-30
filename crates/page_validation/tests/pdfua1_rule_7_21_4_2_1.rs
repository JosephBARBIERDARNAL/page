pub mod common;

const RULE: &str = "PDFUA1-FONT-TYPE1-CHARSET-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.4.2:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-21-4-2-1-complete.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-4-2-1-complete.pdf").to_vec(), || common::pdfua1_rule_7_21_4_2_1_fixture("complete"), &[], false, false, &[]),
        ("pdfua1-rule-7-21-4-2-1-incomplete.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-4-2-1-incomplete.pdf").to_vec(), || common::pdfua1_rule_7_21_4_2_1_fixture("incomplete"), &["PDFUA1-FONT-TYPE1-CHARSET-001"], true, false, &[]),
    ],
}
