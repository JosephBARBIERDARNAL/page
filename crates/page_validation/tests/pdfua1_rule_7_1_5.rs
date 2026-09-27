pub mod common;

const RULE: &str = "PDFUA1-STRUCT-TREE-ROLE-MAP-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-1-5-indirect-mapping.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-5-indirect-mapping.pdf").to_vec(),
            || common::pdfua1_rule_7_1_5_fixture("indirect_mapping"),
            &[],
        ),
        (
            "pdfua1-rule-7-1-5-unmapped.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-5-unmapped.pdf").to_vec(),
            || common::pdfua1_rule_7_1_5_fixture("unmapped"),
            &["PDFUA1-STRUCT-TREE-ROLE-MAP-001"],
        ),
    ],
}
