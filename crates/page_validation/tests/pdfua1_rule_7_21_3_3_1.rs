pub mod common;

const RULE: &str = "PDFUA1-CMAP-EMBEDDING-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-21-3-3-1-embedded.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-3-3-1-embedded.pdf").to_vec(),
            || common::pdfua1_rule_7_21_3_3_1_fixture("embedded"),
            &[],
        ),
        (
            "pdfua1-rule-7-21-3-3-1-predefined.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-3-3-1-predefined.pdf").to_vec(),
            || common::pdfua1_rule_7_21_3_3_1_fixture("predefined"),
            &["PDFUA1-FONT-GLYPH-PRESENCE-001"],
        ),
        (
            "pdfua1-rule-7-21-3-3-1-unembedded.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-3-3-1-unembedded.pdf").to_vec(),
            || common::pdfua1_rule_7_21_3_3_1_fixture("unembedded"),
            &[
                "PDFUA1-CMAP-EMBEDDING-001",
                "PDFUA1-TYPE0-CID-SYSTEM-INFO-001",
            ],
        ),
    ],
}

#[test]
fn reports_unembedded_cmaps_referenced_through_use_cmap() {
    common::assert_pdfua1_rule_behavior(
        RULE,
        &common::pdfua1_rule_7_21_3_3_3_fixture("dictionary_name_unknown"),
        &["PDFUA1-CMAP-EMBEDDING-001", "PDFUA1-CMAP-REFERENCE-001"],
    );
}
