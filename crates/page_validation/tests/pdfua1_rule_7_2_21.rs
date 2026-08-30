pub mod common;

const RULE: &str = "PDFUA1-ACTUAL-TEXT-LANGUAGE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:21";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-2-21-language-missing.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-21-language-missing.pdf").to_vec(), || common::pdfua1_rule_7_2_21_fixture("language_missing"), &["PDFUA1-ACTUAL-TEXT-LANGUAGE-001", "PDFUA1-TEXT-LANGUAGE-001"], true, false, &[]),
        ("pdfua1-rule-7-2-21-language-present.pdf", || include_bytes!("fixtures/pdfua1-rule-7-2-21-language-present.pdf").to_vec(), || common::pdfua1_rule_7_2_21_fixture("language_present"), &["PDFUA1-TEXT-LANGUAGE-001"], false, false, &[]),
    ],
}
