pub mod common;

const RULE: &str = "PDFUA1-ANNOTATION-CONTENTS-ALT-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-18-1-2-alt.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-1-2-alt.pdf").to_vec(),
            || common::pdfua1_rule_7_18_1_2_fixture("alt"),
            &[],
        ),
        (
            "pdfua1-rule-7-18-1-2-contents.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-1-2-contents.pdf").to_vec(),
            || common::pdfua1_rule_7_18_1_2_fixture("contents"),
            &[],
        ),
        (
            "pdfua1-rule-7-18-1-2-empty-contents.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-1-2-empty-contents.pdf").to_vec(),
            || common::pdfua1_rule_7_18_1_2_fixture("empty_contents"),
            &["PDFUA1-ANNOTATION-CONTENTS-ALT-001"],
        ),
        (
            "pdfua1-rule-7-18-1-2-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-1-2-missing.pdf").to_vec(),
            || common::pdfua1_rule_7_18_1_2_fixture("missing"),
            &["PDFUA1-ANNOTATION-CONTENTS-ALT-001"],
        ),
    ],
}
