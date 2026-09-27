pub mod common;

const RULE: &str = "PDFUA1-TBODY-PARENT-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-2-6-contained.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-6-contained.pdf").to_vec(),
            || common::pdfua1_rule_7_2_6_fixture("contained"),
            &[],
        ),
        (
            "pdfua1-rule-7-2-6-not-contained.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-6-not-contained.pdf").to_vec(),
            || common::pdfua1_rule_7_2_6_fixture("not_contained"),
            &["PDFUA1-TBODY-PARENT-001"],
        ),
    ],
}
