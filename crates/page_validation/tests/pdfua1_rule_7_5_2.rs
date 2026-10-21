pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.5:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-5-2-scope-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-5-2-scope-missing.pdf").to_vec(),
            || common::pdfua1_rule_7_5_2_fixture("scope_missing"),
            &["ISO 14289-1:2014:7.5:2"],
        ),
        (
            "pdfua1-rule-7-5-2-scope-present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-5-2-scope-present.pdf").to_vec(),
            || common::pdfua1_rule_7_5_2_fixture("scope_present"),
            &[],
        ),
    ],
}
