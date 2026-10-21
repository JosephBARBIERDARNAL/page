pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.21.3.1:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-21-3-1-identity.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-3-1-identity.pdf").to_vec(),
            || common::pdfua1_rule_7_21_3_1_fixture("identity"),
            &[],
        ),
        (
            "pdfua1-rule-7-21-3-1-matching.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-3-1-matching.pdf").to_vec(),
            || common::pdfua1_rule_7_21_3_1_fixture("matching"),
            &[],
        ),
        (
            "pdfua1-rule-7-21-3-1-registry-mismatch.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-3-1-registry-mismatch.pdf").to_vec(),
            || common::pdfua1_rule_7_21_3_1_fixture("registry_mismatch"),
            &["ISO 14289-1:2014:7.21.3.1:1"],
        ),
        (
            "pdfua1-rule-7-21-3-1-supplement-mismatch.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-3-1-supplement-mismatch.pdf").to_vec(),
            || common::pdfua1_rule_7_21_3_1_fixture("supplement_mismatch"),
            &["ISO 14289-1:2014:7.21.3.1:1"],
        ),
    ],
}

#[test]
fn reports_missing_system_info_for_unembedded_non_table_cmap() {
    common::assert_pdfua1_rule_behavior(
        RULE,
        &common::pdfua1_rule_7_21_3_1_fixture("unembedded_non_table_cmap"),
        &["ISO 14289-1:2014:7.21.3.3:1", "ISO 14289-1:2014:7.21.3.1:1"],
    );
}
