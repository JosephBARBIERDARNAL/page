pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.1:10";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-1-10-false.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-10-false.pdf").to_vec(),
            || common::pdfua1_rule_7_1_10_fixture("false"),
            &[
                "ISO 14289-1:2014:7.1:10",
                "ISO 14289-1:2014:7.2:33",
            ],
        ),
        (
            "pdfua1-rule-7-1-10-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-10-missing.pdf").to_vec(),
            || common::pdfua1_rule_7_1_10_fixture("missing"),
            &[
                "ISO 14289-1:2014:7.1:10",
                "ISO 14289-1:2014:7.2:33",
            ],
        ),
        (
            "pdfua1-rule-7-1-10-present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-10-present.pdf").to_vec(),
            || common::pdfua1_rule_7_1_10_fixture("present"),
            &["ISO 14289-1:2014:7.2:33"],
        ),
    ],
}
