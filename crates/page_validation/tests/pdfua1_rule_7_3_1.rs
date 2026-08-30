pub mod common;

const RULE: &str = "PDFUA1-FIGURE-ALTERNATIVE-TEXT-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.3:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-3-1-actual-text-present.pdf", || include_bytes!("fixtures/pdfua1-rule-7-3-1-actual-text-present.pdf").to_vec(), || common::pdfua1_rule_7_3_1_fixture("actual_text_present"), &[], false, false, &[]),
        ("pdfua1-rule-7-3-1-alt-empty.pdf", || include_bytes!("fixtures/pdfua1-rule-7-3-1-alt-empty.pdf").to_vec(), || common::pdfua1_rule_7_3_1_fixture("alt_empty"), &["PDFUA1-FIGURE-ALTERNATIVE-TEXT-001"], true, false, &[]),
        ("pdfua1-rule-7-3-1-alt-present.pdf", || include_bytes!("fixtures/pdfua1-rule-7-3-1-alt-present.pdf").to_vec(), || common::pdfua1_rule_7_3_1_fixture("alt_present"), &[], false, false, &[]),
        ("pdfua1-rule-7-3-1-missing.pdf", || include_bytes!("fixtures/pdfua1-rule-7-3-1-missing.pdf").to_vec(), || common::pdfua1_rule_7_3_1_fixture("missing"), &["PDFUA1-FIGURE-ALTERNATIVE-TEXT-001"], true, false, &[]),
    ],
}
