pub mod common;

const RULE: &str = "PDFUA1-CONTENT-TAGGING-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.1:3";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-1-3-artifact.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-3-artifact.pdf").to_vec(), || common::pdfua1_rule_7_1_3_fixture("artifact"), &[], false, false, &[]),
        ("pdfua1-rule-7-1-3-tagged.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-3-tagged.pdf").to_vec(), || common::pdfua1_rule_7_1_3_fixture("tagged"), &[], false, false, &[]),
        ("pdfua1-rule-7-1-3-untagged.pdf", || include_bytes!("fixtures/pdfua1-rule-7-1-3-untagged.pdf").to_vec(), || common::pdfua1_rule_7_1_3_fixture("untagged"), &["PDFUA1-CONTENT-TAGGING-001"], true, false, &[]),
    ],
}
