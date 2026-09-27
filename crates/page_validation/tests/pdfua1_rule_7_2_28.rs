pub mod common;

const RULE: &str = "PDFUA1-TOC-CAPTION-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-2-28-caption-first.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-28-caption-first.pdf").to_vec(),
            || common::pdfua1_rule_7_2_28_fixture("caption_first"),
            &[],
        ),
        (
            "pdfua1-rule-7-2-28-caption-not-first.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-28-caption-not-first.pdf").to_vec(),
            || common::pdfua1_rule_7_2_28_fixture("caption_not_first"),
            &["PDFUA1-TOC-CAPTION-001"],
        ),
    ],
}
