pub mod common;

const RULE: &str = "PDFUA1-HEADING-STRUCTURE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.4.4:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-4-4-2-h-only.pdf", || include_bytes!("fixtures/pdfua1-rule-7-4-4-2-h-only.pdf").to_vec(), || common::pdfua1_rule_7_4_4_2_fixture("h_only"), &[], false, false, &[]),
        ("pdfua1-rule-7-4-4-2-h-then-hn.pdf", || include_bytes!("fixtures/pdfua1-rule-7-4-4-2-h-then-hn.pdf").to_vec(), || common::pdfua1_rule_7_4_4_2_fixture("h_then_hn"), &["PDFUA1-HEADING-STRUCTURE-001"], true, false, &[]),
        ("pdfua1-rule-7-4-4-2-hn-only.pdf", || include_bytes!("fixtures/pdfua1-rule-7-4-4-2-hn-only.pdf").to_vec(), || common::pdfua1_rule_7_4_4_2_fixture("hn_only"), &[], false, false, &[]),
        ("pdfua1-rule-7-4-4-2-hn-then-h.pdf", || include_bytes!("fixtures/pdfua1-rule-7-4-4-2-hn-then-h.pdf").to_vec(), || common::pdfua1_rule_7_4_4_2_fixture("hn_then_h"), &["PDFUA1-HEADING-STRUCTURE-001"], true, false, &[]),
    ],
}
