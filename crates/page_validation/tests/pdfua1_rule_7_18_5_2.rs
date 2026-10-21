pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.18.5:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-18-5-2-allowed.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-5-2-allowed.pdf").to_vec(),
            || common::pdfua1_rule_7_18_5_2_fixture("allowed"),
            &[],
        ),
        (
            "pdfua1-rule-7-18-5-2-empty-contents.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-5-2-empty-contents.pdf").to_vec(),
            || common::pdfua1_rule_7_18_5_2_fixture("empty_contents"),
            &[
                "ISO 14289-1:2014:7.18.5:2",
                "ISO 14289-1:2014:7.18.1:2",
            ],
        ),
        (
            "pdfua1-rule-7-18-5-2-hidden.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-5-2-hidden.pdf").to_vec(),
            || common::pdfua1_rule_7_18_5_2_fixture("hidden"),
            &[],
        ),
        (
            "pdfua1-rule-7-18-5-2-missing.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-5-2-missing.pdf").to_vec(),
            || common::pdfua1_rule_7_18_5_2_fixture("missing"),
            &[
                "ISO 14289-1:2014:7.18.5:2",
                "ISO 14289-1:2014:7.18.1:2",
            ],
        ),
        (
            "pdfua1-rule-7-18-5-2-outside-crop-box.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-5-2-outside-crop-box.pdf").to_vec(),
            || common::pdfua1_rule_7_18_5_2_fixture("outside_crop_box"),
            &[],
        ),
    ],
}
