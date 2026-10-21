pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.1:8";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-1-8-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-8-missing.pdf").to_vec(),
            || common::pdfua1_rule_7_1_8_fixture("missing"),
            &["ISO 14289-1:2014:5:2", "ISO 14289-1:2014:5:1", "ISO 14289-1:2014:7.1:8", "ISO 14289-1:2014:7.1:9"],
        ),
        (
            "pdfua1-rule-7-1-8-valid.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-8-valid.pdf").to_vec(),
            || common::pdfua1_rule_7_1_8_fixture("valid"),
            &["ISO 14289-1:2014:7.2:33"],
        ),
        (
            "pdfua1-rule-7-1-8-wrong-subtype.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-8-wrong-subtype.pdf").to_vec(),
            || common::pdfua1_rule_7_1_8_fixture("wrong_subtype"),
            &["ISO 14289-1:2014:7.1:8", "ISO 14289-1:2014:7.2:33"],
        ),
        (
            "pdfua1-rule-7-1-8-wrong-type.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-1-8-wrong-type.pdf").to_vec(),
            || common::pdfua1_rule_7_1_8_fixture("wrong_type"),
            &["ISO 14289-1:2014:7.1:8", "ISO 14289-1:2014:7.2:33"],
        ),
    ],
}
