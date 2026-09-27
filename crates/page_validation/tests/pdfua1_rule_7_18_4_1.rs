pub mod common;

const RULE: &str = "PDFUA1-WIDGET-FORM-TAG-001";

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
            &["PDFUA1-WIDGET-FORM-TAG-001"],
        ),
        (
            "pdfua1-rule-7-18-4-1-role-mapped.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-4-1-role-mapped.pdf").to_vec(),
            || common::pdfua1_rule_7_18_4_1_fixture("role_mapped"),
            &[],
        ),
    ],
}
