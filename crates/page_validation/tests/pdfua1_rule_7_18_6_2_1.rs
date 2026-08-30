pub mod common;

const RULE: &str = "PDFUA1-MEDIA-CLIP-CT-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.18.6.2:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-18-6-2-1-allowed.pdf", || include_bytes!("fixtures/pdfua1-rule-7-18-6-2-1-allowed.pdf").to_vec(), || common::pdfua1_rule_7_18_6_2_1_fixture("allowed"), &[], false, false, &[]),
        ("pdfua1-rule-7-18-6-2-1-missing-ct.pdf", || include_bytes!("fixtures/pdfua1-rule-7-18-6-2-1-missing-ct.pdf").to_vec(), || common::pdfua1_rule_7_18_6_2_1_fixture("missing_ct"), &["PDFUA1-MEDIA-CLIP-CT-001"], true, false, &[]),
    ],
}
