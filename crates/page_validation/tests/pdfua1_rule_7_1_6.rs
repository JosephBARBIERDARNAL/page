pub mod common;

const RULE: &str = "PDFUA1-STRUCT-TREE-ROLE-MAP-CYCLE-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-1-6-acyclic-mapping.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-6-acyclic-mapping.pdf").to_vec(),
            || common::pdfua1_rule_7_1_6_fixture("acyclic_mapping"),
            &[],
        ),
        (
            "pdfua1-rule-7-1-6-circular-mapping.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-6-circular-mapping.pdf").to_vec(),
            || common::pdfua1_rule_7_1_6_fixture("circular_mapping"),
            &["PDFUA1-STRUCT-TREE-ROLE-MAP-CYCLE-001"],
        ),
    ],
}
