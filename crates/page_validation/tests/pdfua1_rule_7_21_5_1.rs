pub mod common;

const RULE: &str = "PDFUA1-FONT-GLYPH-WIDTH-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-21-5-1-matching.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-5-1-matching.pdf").to_vec(),
            || common::pdfua1_rule_7_21_5_1_fixture("matching"),
            &[],
        ),
        (
            "pdfua1-rule-7-21-5-1-mismatched.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-5-1-mismatched.pdf").to_vec(),
            || common::pdfua1_rule_7_21_5_1_fixture("mismatched"),
            &["PDFUA1-FONT-GLYPH-WIDTH-001"],
        ),
    ],
}
