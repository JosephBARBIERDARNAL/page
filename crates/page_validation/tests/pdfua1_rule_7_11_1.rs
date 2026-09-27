pub mod common;

const RULE: &str = "PDFUA1-FILE-SPEC-F-AND-UF-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-11-1-empty-uf.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-11-1-empty-uf.pdf").to_vec(),
            || common::pdfua1_rule_7_11_1_fixture("empty_uf"),
            &["PDFUA1-FILE-SPEC-F-AND-UF-001"],
        ),
        (
            "pdfua1-rule-7-11-1-valid.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-11-1-valid.pdf").to_vec(),
            || common::pdfua1_rule_7_11_1_fixture("valid"),
            &[],
        ),
    ],
}
