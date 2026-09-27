pub mod common;

const RULE: &str = "PDFUA1-ID-PART-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-5-2-present.pdf",
            || include_bytes!("fixtures/pdfua1-rule-5-2-present.pdf").to_vec(),
            || common::pdfua1_rule_5_2_fixture("present"),
            &[],
        ),
        (
            "pdfua1-rule-5-2-wrong-part.pdf",
            || include_bytes!("fixtures/pdfua1-rule-5-2-wrong-part.pdf").to_vec(),
            || common::pdfua1_rule_5_2_fixture("wrong_part"),
            &["PDFUA1-ID-PART-001"],
        ),
    ],
}
