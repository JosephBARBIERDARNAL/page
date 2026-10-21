pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.21.3.3:1";

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
            &["ISO 14289-1:2014:7.21.4.1:2"],
        ),
        (
            "pdfua1-rule-7-21-3-3-1-unembedded.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-3-3-1-unembedded.pdf").to_vec(),
            || common::pdfua1_rule_7_21_3_3_1_fixture("unembedded"),
            &[
                "ISO 14289-1:2014:7.21.3.3:1",
                "ISO 14289-1:2014:7.21.3.1:1",
            ],
        ),
    ],
}

#[test]
fn reports_unembedded_cmaps_referenced_through_use_cmap() {
    common::assert_pdfua1_rule_behavior(
        RULE,
        &common::pdfua1_rule_7_21_3_3_3_fixture("dictionary_name_unknown"),
        &["ISO 14289-1:2014:7.21.3.3:1", "ISO 14289-1:2014:7.21.3.3:3"],
    );
}

#[test]
fn accepts_embedded_cmap_stream_as_usecmap_base() {
    common::assert_pdfua1_rule_behavior(
        RULE,
        &common::pdfua1_rule_7_21_3_3_3_fixture("dictionary_embedded_unknown"),
        &["ISO 14289-1:2014:7.21.3.3:3"],
    );
}

#[test]
fn reports_unembedded_usecmap_when_dictionary_stream_name_does_not_match() {
    common::assert_pdfua1_rule_behavior(
        RULE,
        &common::pdfua1_rule_7_21_3_3_3_fixture("dictionary_embedded_mismatch"),
        &["ISO 14289-1:2014:7.21.3.3:1", "ISO 14289-1:2014:7.21.3.3:3"],
    );
}
