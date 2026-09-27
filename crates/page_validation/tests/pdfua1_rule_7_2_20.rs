pub mod common;

const RULE: &str = "PDFUA1-LI-KIDS-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-2-20-allowed.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-20-allowed.pdf").to_vec(),
            || common::pdfua1_rule_7_2_20_fixture("allowed"),
            &[],
        ),
        (
            "pdfua1-rule-7-2-20-invalid.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-20-invalid.pdf").to_vec(),
            || common::pdfua1_rule_7_2_20_fixture("invalid"),
            &["PDFUA1-LI-KIDS-001"],
        ),
    ],
}
