pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.21.7:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-21-7-2-feff.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-7-2-feff.pdf").to_vec(),
            || common::pdfua1_rule_7_21_7_2_fixture("feff"),
            &["ISO 14289-1:2014:7.21.7:2"],
        ),
        (
            "pdfua1-rule-7-21-7-2-fffe.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-7-2-fffe.pdf").to_vec(),
            || common::pdfua1_rule_7_21_7_2_fixture("fffe"),
            &["ISO 14289-1:2014:7.21.7:2"],
        ),
        (
            "pdfua1-rule-7-21-7-2-matching.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-7-2-matching.pdf").to_vec(),
            || common::pdfua1_rule_7_21_7_2_fixture("matching"),
            &[],
        ),
        (
            "pdfua1-rule-7-21-7-2-zero.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-7-2-zero.pdf").to_vec(),
            || common::pdfua1_rule_7_21_7_2_fixture("zero"),
            &["ISO 14289-1:2014:7.21.7:2"],
        ),
    ],
}
