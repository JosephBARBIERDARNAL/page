pub mod common;

const RULE: &str = "PDFUA1-PRINTER-MARK-ARTIFACT-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.18.8:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-18-8-1-allowed.pdf", || include_bytes!("fixtures/pdfua1-rule-7-18-8-1-allowed.pdf").to_vec(), || common::pdfua1_rule_7_18_8_1_fixture("allowed"), &[], false, false, &[]),
        ("pdfua1-rule-7-18-8-1-hidden.pdf", || include_bytes!("fixtures/pdfua1-rule-7-18-8-1-hidden.pdf").to_vec(), || common::pdfua1_rule_7_18_8_1_fixture("hidden"), &[], false, false, &[]),
        ("pdfua1-rule-7-18-8-1-included.pdf", || include_bytes!("fixtures/pdfua1-rule-7-18-8-1-included.pdf").to_vec(), || common::pdfua1_rule_7_18_8_1_fixture("included"), &["PDFUA1-PRINTER-MARK-ARTIFACT-001"], true, false, &[]),
        ("pdfua1-rule-7-18-8-1-outside-crop-box.pdf", || include_bytes!("fixtures/pdfua1-rule-7-18-8-1-outside-crop-box.pdf").to_vec(), || common::pdfua1_rule_7_18_8_1_fixture("outside_crop_box"), &[], false, false, &[]),
    ],
}
