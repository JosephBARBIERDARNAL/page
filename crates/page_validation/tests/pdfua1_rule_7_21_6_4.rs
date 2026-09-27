pub mod common;

const RULE: &str = "PDFUA1-TRUETYPE-SYMBOLIC-CMAP-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-21-6-4-one-cmap.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-6-4-one-cmap.pdf").to_vec(),
            || common::pdfua1_rule_7_21_6_4_fixture("one_cmap"),
            &[],
        ),
        (
            "pdfua1-rule-7-21-6-4-two-cmaps-with-cmap30.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-6-4-two-cmaps-with-cmap30.pdf").to_vec(),
            || common::pdfua1_rule_7_21_6_4_fixture("two_cmaps_with_cmap30"),
            &[],
        ),
        (
            "pdfua1-rule-7-21-6-4-two-cmaps.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-6-4-two-cmaps.pdf").to_vec(),
            || common::pdfua1_rule_7_21_6_4_fixture("two_cmaps"),
            &["PDFUA1-TRUETYPE-SYMBOLIC-CMAP-001"],
        ),
    ],
}
