pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.4.4:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-4-4-2-h-only.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-4-4-2-h-only.pdf").to_vec(),
            || common::pdfua1_rule_7_4_4_2_fixture("h_only"),
            &[],
        ),
        (
            "pdfua1-rule-7-4-4-2-h-then-hn.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-4-4-2-h-then-hn.pdf").to_vec(),
            || common::pdfua1_rule_7_4_4_2_fixture("h_then_hn"),
            &["ISO 14289-1:2014:7.4.4:2"],
        ),
        (
            "pdfua1-rule-7-4-4-2-hn-only.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-4-4-2-hn-only.pdf").to_vec(),
            || common::pdfua1_rule_7_4_4_2_fixture("hn_only"),
            &[],
        ),
        (
            "pdfua1-rule-7-4-4-2-hn-then-h.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-4-4-2-hn-then-h.pdf").to_vec(),
            || common::pdfua1_rule_7_4_4_2_fixture("hn_then_h"),
            &["ISO 14289-1:2014:7.4.4:2"],
        ),
    ],
}
