pub mod common;

const RULE: &str = "PDFUA1-ARTIFACT-NESTED-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-1-1-inside-tagged-content.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-1-inside-tagged-content.pdf").to_vec(),
            || common::pdfua1_rule_7_1_1_fixture("inside_tagged_content"),
            &["PDFUA1-ARTIFACT-NESTED-001"],
        ),
        (
            "pdfua1-rule-7-1-1-outside-tagged-content.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-1-outside-tagged-content.pdf").to_vec(),
            || common::pdfua1_rule_7_1_1_fixture("outside_tagged_content"),
            &[],
        ),
    ],
}
