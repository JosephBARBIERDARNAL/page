pub mod common;

const RULE: &str = "PDFUA1-TABLE-COLUMN-ROWSPAN-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-2-41-allowed.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-41-allowed.pdf").to_vec(),
            || common::pdfua1_rule_7_2_41_fixture("allowed"),
            &[],
        ),
        (
            "pdfua1-rule-7-2-41-invalid.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-41-invalid.pdf").to_vec(),
            || common::pdfua1_rule_7_2_41_fixture("invalid"),
            &["PDFUA1-TABLE-COLUMN-ROWSPAN-001"],
        ),
    ],
}
