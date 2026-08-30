pub mod common;

const RULE: &str = "PDFUA1-DYNAMIC-XFA-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.15:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-15-1-dynamic-xfa.pdf", || include_bytes!("fixtures/pdfua1-rule-7-15-1-dynamic-xfa.pdf").to_vec(), || common::pdfua1_rule_7_15_1_fixture("dynamic_xfa"), &["PDFUA1-DYNAMIC-XFA-001"], true, false, &[]),
        ("pdfua1-rule-7-15-1-no-xfa.pdf", || include_bytes!("fixtures/pdfua1-rule-7-15-1-no-xfa.pdf").to_vec(), || common::pdfua1_rule_7_15_1_fixture("no_xfa"), &[], false, false, &[]),
        ("pdfua1-rule-7-15-1-static-xfa.pdf", || include_bytes!("fixtures/pdfua1-rule-7-15-1-static-xfa.pdf").to_vec(), || common::pdfua1_rule_7_15_1_fixture("static_xfa"), &[], false, false, &[]),
    ],
}
