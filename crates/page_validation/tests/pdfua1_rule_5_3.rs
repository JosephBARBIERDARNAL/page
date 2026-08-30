pub mod common;

const RULE: &str = "PDFUA1-ID-PART-PREFIX-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:5:3";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-5-3-canonical-prefix.pdf", || include_bytes!("fixtures/pdfua1-rule-5-3-canonical-prefix.pdf").to_vec(), || common::pdfua1_rule_5_3_fixture("canonical_prefix"), &[], false, false, &[]),
        ("pdfua1-rule-5-3-wrong-prefix.pdf", || include_bytes!("fixtures/pdfua1-rule-5-3-wrong-prefix.pdf").to_vec(), || common::pdfua1_rule_5_3_fixture("wrong_prefix"), &["PDFUA1-ID-PART-PREFIX-001"], true, false, &[]),
    ],
}
