pub mod common;

const RULE: &str = "PDFUA1-HEADING-NESTING-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.4.2:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-4-2-1-first-heading-h2.pdf", || include_bytes!("fixtures/pdfua1-rule-7-4-2-1-first-heading-h2.pdf").to_vec(), || common::pdfua1_rule_7_4_2_1_fixture("first_heading_h2"), &["PDFUA1-HEADING-NESTING-001"], true, false, &[]),
        ("pdfua1-rule-7-4-2-1-skips-h2.pdf", || include_bytes!("fixtures/pdfua1-rule-7-4-2-1-skips-h2.pdf").to_vec(), || common::pdfua1_rule_7_4_2_1_fixture("skips_h2"), &["PDFUA1-HEADING-NESTING-001"], true, false, &[]),
        ("pdfua1-rule-7-4-2-1-valid.pdf", || include_bytes!("fixtures/pdfua1-rule-7-4-2-1-valid.pdf").to_vec(), || common::pdfua1_rule_7_4_2_1_fixture("valid"), &[], false, false, &[]),
    ],
}
