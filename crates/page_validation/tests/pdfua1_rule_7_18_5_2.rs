pub mod common;

const RULE: &str = "PDFUA1-LINK-CONTENTS-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.18.5:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-18-5-2-allowed.pdf", || include_bytes!("fixtures/pdfua1-rule-7-18-5-2-allowed.pdf").to_vec(), || common::pdfua1_rule_7_18_5_2_fixture("allowed"), &[], false, false, &[]),
        ("pdfua1-rule-7-18-5-2-empty-contents.pdf", || include_bytes!("fixtures/pdfua1-rule-7-18-5-2-empty-contents.pdf").to_vec(), || common::pdfua1_rule_7_18_5_2_fixture("empty_contents"), &["PDFUA1-LINK-CONTENTS-001"], true, false, &[]),
        ("pdfua1-rule-7-18-5-2-hidden.pdf", || include_bytes!("fixtures/pdfua1-rule-7-18-5-2-hidden.pdf").to_vec(), || common::pdfua1_rule_7_18_5_2_fixture("hidden"), &[], false, false, &[]),
        ("pdfua1-rule-7-18-5-2-missing.pdf", || include_bytes!("fixtures/pdfua1-rule-7-18-5-2-missing.pdf").to_vec(), || common::pdfua1_rule_7_18_5_2_fixture("missing"), &["PDFUA1-LINK-CONTENTS-001"], true, false, &[]),
        ("pdfua1-rule-7-18-5-2-outside-crop-box.pdf", || include_bytes!("fixtures/pdfua1-rule-7-18-5-2-outside-crop-box.pdf").to_vec(), || common::pdfua1_rule_7_18_5_2_fixture("outside_crop_box"), &[], false, false, &[]),
    ],
}
