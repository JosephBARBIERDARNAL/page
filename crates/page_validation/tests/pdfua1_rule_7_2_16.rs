pub mod common;

const RULE: &str = "PDFUA1-TABLE-CAPTION-POSITION-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-2-16-caption-first.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-16-caption-first.pdf").to_vec(),
            || common::pdfua1_rule_7_2_16_fixture("caption_first"),
            &[],
        ),
        (
            "pdfua1-rule-7-2-16-caption-last.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-16-caption-last.pdf").to_vec(),
            || common::pdfua1_rule_7_2_16_fixture("caption_last"),
            &[],
        ),
        (
            "pdfua1-rule-7-2-16-caption-middle.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-16-caption-middle.pdf").to_vec(),
            || common::pdfua1_rule_7_2_16_fixture("caption_middle"),
            &["PDFUA1-TABLE-CAPTION-POSITION-001"],
        ),
    ],
}
