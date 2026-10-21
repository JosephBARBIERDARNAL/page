pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.18.4:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-18-4-1-allowed.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-4-1-allowed.pdf").to_vec(),
            || common::pdfua1_rule_7_18_4_1_fixture("allowed"),
            &[],
        ),
        (
            "pdfua1-rule-7-18-4-1-not-nested.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-4-1-not-nested.pdf").to_vec(),
            || common::pdfua1_rule_7_18_4_1_fixture("not_nested"),
            &["ISO 14289-1:2014:7.18.4:1"],
        ),
        (
            "pdfua1-rule-7-18-4-1-role-mapped.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-4-1-role-mapped.pdf").to_vec(),
            || common::pdfua1_rule_7_18_4_1_fixture("role_mapped"),
            &[],
        ),
    ],
}
