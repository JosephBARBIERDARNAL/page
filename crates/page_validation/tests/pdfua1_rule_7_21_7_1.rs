pub mod common;

const RULE: &str = "PDFUA1-FONT-UNICODE-MAPPING-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-21-7-1-matching.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-7-1-matching.pdf").to_vec(),
            || common::pdfua1_rule_7_21_7_1_fixture("matching"),
            &[],
        ),
        (
            "pdfua1-rule-7-21-7-1-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-7-1-missing.pdf").to_vec(),
            || common::pdfua1_rule_7_21_7_1_fixture("missing"),
            &["PDFUA1-FONT-UNICODE-MAPPING-001"],
        ),
    ],
}
