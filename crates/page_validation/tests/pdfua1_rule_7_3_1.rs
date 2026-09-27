pub mod common;

const RULE: &str = "PDFUA1-FIGURE-ALTERNATIVE-TEXT-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-3-1-actual-text-present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-3-1-actual-text-present.pdf").to_vec(),
            || common::pdfua1_rule_7_3_1_fixture("actual_text_present"),
            &[],
        ),
        (
            "pdfua1-rule-7-3-1-alt-empty.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-3-1-alt-empty.pdf").to_vec(),
            || common::pdfua1_rule_7_3_1_fixture("alt_empty"),
            &["PDFUA1-FIGURE-ALTERNATIVE-TEXT-001"],
        ),
        (
            "pdfua1-rule-7-3-1-alt-present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-3-1-alt-present.pdf").to_vec(),
            || common::pdfua1_rule_7_3_1_fixture("alt_present"),
            &[],
        ),
        (
            "pdfua1-rule-7-3-1-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-3-1-missing.pdf").to_vec(),
            || common::pdfua1_rule_7_3_1_fixture("missing"),
            &["PDFUA1-FIGURE-ALTERNATIVE-TEXT-001"],
        ),
    ],
}
