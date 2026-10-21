pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.21.3.2:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-21-3-2-identity.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-3-2-identity.pdf").to_vec(),
            || common::pdfua1_rule_7_21_3_2_fixture("identity"),
            &[],
        ),
        (
            "pdfua1-rule-7-21-3-2-invalid.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-3-2-invalid.pdf").to_vec(),
            || common::pdfua1_rule_7_21_3_2_fixture("invalid"),
            &["ISO 14289-1:2014:7.21.3.2:1"],
        ),
        (
            "pdfua1-rule-7-21-3-2-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-3-2-missing.pdf").to_vec(),
            || common::pdfua1_rule_7_21_3_2_fixture("missing"),
            &["ISO 14289-1:2014:7.21.3.2:1"],
        ),
        (
            "pdfua1-rule-7-21-3-2-stream.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-3-2-stream.pdf").to_vec(),
            || common::pdfua1_rule_7_21_3_2_fixture("stream"),
            &[],
        ),
    ],
}
