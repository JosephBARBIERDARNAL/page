pub mod common;

const RULE: &str = "PDFUA1-MEDIA-CLIP-ALT-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.18.6.2:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-18-6-2-2-allowed.pdf", || include_bytes!("fixtures/pdfua1-rule-7-18-6-2-2-allowed.pdf").to_vec(), || common::pdfua1_rule_7_18_6_2_1_fixture("allowed"), &[], false, false, &[]),
        ("pdfua1-rule-7-18-6-2-2-invalid-alt.pdf", || include_bytes!("fixtures/pdfua1-rule-7-18-6-2-2-invalid-alt.pdf").to_vec(), || common::pdfua1_rule_7_18_6_2_1_fixture("invalid_alt"), &["PDFUA1-MEDIA-CLIP-ALT-001"], true, false, &[]),
        ("pdfua1-rule-7-18-6-2-2-missing-alt.pdf", || include_bytes!("fixtures/pdfua1-rule-7-18-6-2-2-missing-alt.pdf").to_vec(), || common::pdfua1_rule_7_18_6_2_1_fixture("missing_alt"), &["PDFUA1-MEDIA-CLIP-ALT-001"], true, false, &[]),
    ],
}
