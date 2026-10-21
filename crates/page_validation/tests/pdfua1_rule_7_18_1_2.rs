pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.18.1:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-18-1-2-alt.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-1-2-alt.pdf").to_vec(),
            || common::pdfua1_rule_7_18_1_2_fixture("alt"),
            &[],
        ),
        (
            "pdfua1-rule-7-18-1-2-contents.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-1-2-contents.pdf").to_vec(),
            || common::pdfua1_rule_7_18_1_2_fixture("contents"),
            &[],
        ),
        (
            "pdfua1-rule-7-18-1-2-empty-contents.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-1-2-empty-contents.pdf").to_vec(),
            || common::pdfua1_rule_7_18_1_2_fixture("empty_contents"),
            &["ISO 14289-1:2014:7.18.1:2"],
        ),
        (
            "pdfua1-rule-7-18-1-2-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-1-2-missing.pdf").to_vec(),
            || common::pdfua1_rule_7_18_1_2_fixture("missing"),
            &["ISO 14289-1:2014:7.18.1:2"],
        ),
        (
            "pdfua1-rule-7-18-1-2-link-missing-contents-and-alt.pdf",
            || common::pdfua1_rule_7_18_1_2_fixture("link_missing_contents_and_alt"),
            || common::pdfua1_rule_7_18_1_2_fixture("link_missing_contents_and_alt"),
            &[
                "ISO 14289-1:2014:7.18.5:2",
                "ISO 14289-1:2014:7.18.1:2",
            ],
        ),
    ],
}
