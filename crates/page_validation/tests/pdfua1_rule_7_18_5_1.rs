pub mod common;

const RULE: &str = "PDFUA1-LINK-LINK-TAG-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-18-5-1-allowed.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-5-1-allowed.pdf").to_vec(),
            || common::pdfua1_rule_7_18_5_1_fixture("allowed"),
            &[],
        ),
        (
            "pdfua1-rule-7-18-5-1-hidden.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-5-1-hidden.pdf").to_vec(),
            || common::pdfua1_rule_7_18_5_1_fixture("hidden"),
            &[],
        ),
        (
            "pdfua1-rule-7-18-5-1-not-nested.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-5-1-not-nested.pdf").to_vec(),
            || common::pdfua1_rule_7_18_5_1_fixture("not_nested"),
            &["PDFUA1-LINK-LINK-TAG-001"],
        ),
        (
            "pdfua1-rule-7-18-5-1-outside-crop-box.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-5-1-outside-crop-box.pdf").to_vec(),
            || common::pdfua1_rule_7_18_5_1_fixture("outside_crop_box"),
            &[],
        ),
        (
            "pdfua1-rule-7-18-5-1-role-mapped.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-18-5-1-role-mapped.pdf").to_vec(),
            || common::pdfua1_rule_7_18_5_1_fixture("role_mapped"),
            &[],
        ),
    ],
}
