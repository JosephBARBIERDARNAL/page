pub mod common;

const RULE: &str = "PDFUA1-ENCRYPTION-P-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.16:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-16-1-bit-10-false.pdf", || include_bytes!("fixtures/pdfua1-rule-7-16-1-bit-10-false.pdf").to_vec(), || common::pdfua1_rule_7_16_1_fixture("bit_10_false"), &["PDFUA1-ENCRYPTION-P-001"], true, false, &[]),
        ("pdfua1-rule-7-16-1-missing-p.pdf", || include_bytes!("fixtures/pdfua1-rule-7-16-1-missing-p.pdf").to_vec(), || common::pdfua1_rule_7_16_1_fixture("missing_p"), &["PDFUA1-ENCRYPTION-P-001", "PDFUA1-ID-PART-001", "PDFUA1-ID-SCHEMA-001", "PDFUA1-METADATA-STRUCTURE-001", "PDFUA1-METADATA-TITLE-001", "PDFUA1-STRUCT-TREE-ROOT-001", "PDFUA1-TAGGED-DOCUMENT-001", "PDFUA1-VIEWER-PREFERENCES-001"], true, true, &[]),
        ("pdfua1-rule-7-16-1-valid.pdf", || include_bytes!("fixtures/pdfua1-rule-7-16-1-valid.pdf").to_vec(), || common::pdfua1_rule_7_16_1_fixture("valid"), &[], false, false, &[]),
    ],
}
