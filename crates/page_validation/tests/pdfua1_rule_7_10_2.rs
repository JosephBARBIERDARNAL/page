pub mod common;

const RULE: &str = "PDFUA1-OPTIONAL-CONTENT-AS-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-10-2-as-present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-10-2-as-present.pdf").to_vec(),
            || common::pdfua1_rule_7_10_2_fixture("as_present"),
            &["PDFUA1-OPTIONAL-CONTENT-AS-001"],
        ),
        (
            "pdfua1-rule-7-10-2-valid.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-10-2-valid.pdf").to_vec(),
            || common::pdfua1_rule_7_10_2_fixture("valid"),
            &[],
        ),
    ],
}
