pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.18.8:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-18-8-1-allowed.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-8-1-allowed.pdf").to_vec(),
            || common::pdfua1_rule_7_18_8_1_fixture("allowed"),
            &[],
        ),
        (
            "pdfua1-rule-7-18-8-1-hidden.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-8-1-hidden.pdf").to_vec(),
            || common::pdfua1_rule_7_18_8_1_fixture("hidden"),
            &[],
        ),
        (
            "pdfua1-rule-7-18-8-1-included.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-8-1-included.pdf").to_vec(),
            || common::pdfua1_rule_7_18_8_1_fixture("included"),
            &["ISO 14289-1:2014:7.18.8:1"],
        ),
        (
            "pdfua1-rule-7-18-8-1-outside-crop-box.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-8-1-outside-crop-box.pdf").to_vec(),
            || common::pdfua1_rule_7_18_8_1_fixture("outside_crop_box"),
            &[],
        ),
    ],
}
