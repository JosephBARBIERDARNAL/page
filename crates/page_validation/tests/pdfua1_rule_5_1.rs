pub mod common;

const RULE: &str = "PDFUA1-ID-SCHEMA-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-5-1-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-5-1-missing.pdf").to_vec(),
            || common::pdfua1_rule_5_1_fixture("missing"),
            &["PDFUA1-ID-PART-001", "PDFUA1-ID-SCHEMA-001"],
        ),
        (
            "pdfua1-rule-5-1-present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-5-1-present.pdf").to_vec(),
            || common::pdfua1_rule_5_1_fixture("present"),
            &[],
        ),
    ],
}
