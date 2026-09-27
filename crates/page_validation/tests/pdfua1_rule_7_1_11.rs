pub mod common;

const RULE: &str = "PDFUA1-STRUCT-TREE-ROOT-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-1-11-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-11-missing.pdf").to_vec(),
            || common::pdfua1_rule_7_1_11_fixture("missing"),
            &["PDFUA1-STRUCT-TREE-ROOT-001"],
        ),
        (
            "pdfua1-rule-7-1-11-present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-11-present.pdf").to_vec(),
            || common::pdfua1_rule_7_1_11_fixture("present"),
            &[],
        ),
    ],
}
