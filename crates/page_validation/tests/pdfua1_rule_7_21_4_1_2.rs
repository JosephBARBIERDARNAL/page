pub mod common;

const RULE: &str = "PDFUA1-FONT-GLYPH-PRESENCE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.4.1:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-21-4-1-2-invisible.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-4-1-2-invisible.pdf").to_vec(), || common::pdfua1_rule_7_21_4_1_2_fixture("invisible"), &["PDFUA1-CONTENT-TAGGING-001"], false, false, &[]),
        ("pdfua1-rule-7-21-4-1-2-missing.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-4-1-2-missing.pdf").to_vec(), || common::pdfua1_rule_7_21_4_1_2_fixture("missing"), &["PDFUA1-CONTENT-TAGGING-001", "PDFUA1-FONT-GLYPH-PRESENCE-001"], true, false, &[]),
        ("pdfua1-rule-7-21-4-1-2-present.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-4-1-2-present.pdf").to_vec(), || common::pdfua1_rule_7_21_4_1_2_fixture("present"), &["PDFUA1-CONTENT-TAGGING-001"], false, false, &[]),
    ],
}
