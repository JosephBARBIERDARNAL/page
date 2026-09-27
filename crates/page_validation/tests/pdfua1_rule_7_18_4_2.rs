pub mod common;

const RULE: &str = "PDFUA1-FORM-CHILDREN-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-18-4-2-allowed.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-4-2-allowed.pdf").to_vec(),
            || common::pdfua1_rule_7_18_4_2_fixture("allowed"),
            &[],
        ),
        (
            "pdfua1-rule-7-18-4-2-invalid.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-4-2-invalid.pdf").to_vec(),
            || common::pdfua1_rule_7_18_4_2_fixture("invalid"),
            &["PDFUA1-FORM-CHILDREN-001"],
        ),
        (
            "pdfua1-rule-7-18-4-2-role-attribute.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-4-2-role-attribute.pdf").to_vec(),
            || common::pdfua1_rule_7_18_4_2_fixture("role_attribute"),
            &[],
        ),
    ],
}
