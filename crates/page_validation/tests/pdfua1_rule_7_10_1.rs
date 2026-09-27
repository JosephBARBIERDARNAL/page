pub mod common;

const RULE: &str = "PDFUA1-OPTIONAL-CONTENT-NAME-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-10-1-missing-config-name.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-10-1-missing-config-name.pdf").to_vec(),
            || common::pdfua1_rule_7_10_1_fixture("missing_config_name"),
            &["PDFUA1-OPTIONAL-CONTENT-NAME-001"],
        ),
        (
            "pdfua1-rule-7-10-1-missing-default-name.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-10-1-missing-default-name.pdf").to_vec(),
            || common::pdfua1_rule_7_10_1_fixture("missing_default_name"),
            &["PDFUA1-OPTIONAL-CONTENT-NAME-001"],
        ),
        (
            "pdfua1-rule-7-10-1-valid.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-10-1-valid.pdf").to_vec(),
            || common::pdfua1_rule_7_10_1_fixture("valid"),
            &[],
        ),
    ],
}
