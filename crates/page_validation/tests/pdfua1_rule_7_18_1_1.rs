pub mod common;

const RULE: &str = "PDFUA1-ANNOTATION-ANNOT-TAG-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-18-1-1-invalid.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-1-1-invalid.pdf").to_vec(),
            || common::pdfua1_rule_7_18_1_1_fixture("invalid"),
            &["PDFUA1-ANNOTATION-ANNOT-TAG-001"],
        ),
        (
            "pdfua1-rule-7-18-1-1-valid.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-1-1-valid.pdf").to_vec(),
            || common::pdfua1_rule_7_18_1_1_fixture("valid"),
            &[],
        ),
    ],
}
