pub mod common;

const RULE: &str = "PDFUA1-CIDTOGIDMAP-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.3.2:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-21-3-2-identity.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-3-2-identity.pdf").to_vec(), || common::pdfua1_rule_7_21_3_2_fixture("identity"), &[], false, false, &[]),
        ("pdfua1-rule-7-21-3-2-invalid.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-3-2-invalid.pdf").to_vec(), || common::pdfua1_rule_7_21_3_2_fixture("invalid"), &["PDFUA1-CIDTOGIDMAP-001"], true, false, &[]),
        ("pdfua1-rule-7-21-3-2-missing.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-3-2-missing.pdf").to_vec(), || common::pdfua1_rule_7_21_3_2_fixture("missing"), &["PDFUA1-CIDTOGIDMAP-001"], true, false, &[]),
        ("pdfua1-rule-7-21-3-2-stream.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-3-2-stream.pdf").to_vec(), || common::pdfua1_rule_7_21_3_2_fixture("stream"), &[], false, false, &[]),
    ],
}
