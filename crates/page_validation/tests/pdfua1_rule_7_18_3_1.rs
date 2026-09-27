pub mod common;

const RULE: &str = "PDFUA1-PAGE-TABS-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-18-3-1-allowed.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-3-1-allowed.pdf").to_vec(),
            || common::pdfua1_rule_7_18_3_1_fixture("allowed"),
            &[],
        ),
        (
            "pdfua1-rule-7-18-3-1-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-3-1-missing.pdf").to_vec(),
            || common::pdfua1_rule_7_18_3_1_fixture("missing"),
            &["PDFUA1-PAGE-TABS-001"],
        ),
        (
            "pdfua1-rule-7-18-3-1-wrong.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-3-1-wrong.pdf").to_vec(),
            || common::pdfua1_rule_7_18_3_1_fixture("wrong"),
            &["PDFUA1-PAGE-TABS-001"],
        ),
    ],
}
