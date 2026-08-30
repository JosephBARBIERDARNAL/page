pub mod common;

const RULE: &str = "PDFUA1-METADATA-STRUCTURE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.1:8";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-1-8-missing.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-8-missing.pdf").to_vec(), || common::pdfua1_rule_7_1_8_fixture("missing"), &["PDFUA1-ID-PART-001", "PDFUA1-ID-SCHEMA-001", "PDFUA1-METADATA-STRUCTURE-001", "PDFUA1-METADATA-TITLE-001"], true, false, &[]),
        ("pdfua1-rule-7-1-8-valid.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-8-valid.pdf").to_vec(), || common::pdfua1_rule_7_1_8_fixture("valid"), &[], false, false, &[]),
        ("pdfua1-rule-7-1-8-wrong-subtype.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-8-wrong-subtype.pdf").to_vec(), || common::pdfua1_rule_7_1_8_fixture("wrong_subtype"), &["PDFUA1-METADATA-STRUCTURE-001"], true, false, &[]),
        ("pdfua1-rule-7-1-8-wrong-type.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-8-wrong-type.pdf").to_vec(), || common::pdfua1_rule_7_1_8_fixture("wrong_type"), &["PDFUA1-METADATA-STRUCTURE-001"], true, false, &[]),
    ],
}
