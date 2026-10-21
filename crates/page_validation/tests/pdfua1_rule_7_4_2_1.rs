pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.4.2:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-4-2-1-first-heading-h2.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-4-2-1-first-heading-h2.pdf").to_vec(),
            || common::pdfua1_rule_7_4_2_1_fixture("first_heading_h2"),
            &["ISO 14289-1:2014:7.4.2:1"],
        ),
        (
            "pdfua1-rule-7-4-2-1-skips-h2.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-4-2-1-skips-h2.pdf").to_vec(),
            || common::pdfua1_rule_7_4_2_1_fixture("skips_h2"),
            &["ISO 14289-1:2014:7.4.2:1"],
        ),
        (
            "pdfua1-rule-7-4-2-1-valid.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-4-2-1-valid.pdf").to_vec(),
            || common::pdfua1_rule_7_4_2_1_fixture("valid"),
            &[],
        ),
    ],
}
