pub mod common;

const RULE: &str = "PDFUA1-STRUCT-TREE-ROLE-MAP-STANDARD-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-1-7-standard-remapped.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-7-standard-remapped.pdf").to_vec(),
            || common::pdfua1_rule_7_1_7_fixture("standard_remapped"),
            &["PDFUA1-STRUCT-TREE-ROLE-MAP-STANDARD-001"],
        ),
        (
            "pdfua1-rule-7-1-7-standard-unmapped.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-7-standard-unmapped.pdf").to_vec(),
            || common::pdfua1_rule_7_1_7_fixture("standard_unmapped"),
            &[],
        ),
    ],
}
