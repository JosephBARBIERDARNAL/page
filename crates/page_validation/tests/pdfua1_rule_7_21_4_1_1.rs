pub mod common;

const RULE: &str = "PDFUA1-FONT-EMBEDDING-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-21-4-1-1-embedded.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-4-1-1-embedded.pdf").to_vec(),
            || common::pdfua1_rule_7_21_4_1_1_fixture("embedded"),
            &[],
        ),
        (
            "pdfua1-rule-7-21-4-1-1-unembedded.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-4-1-1-unembedded.pdf").to_vec(),
            || common::pdfua1_rule_7_21_4_1_1_fixture("unembedded"),
            &["PDFUA1-FONT-EMBEDDING-001"],
        ),
    ],
}
