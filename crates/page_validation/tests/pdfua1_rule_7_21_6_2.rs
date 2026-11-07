pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.21.6:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-21-6-2-invalid-differences.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-6-2-invalid-differences.pdf").to_vec(),
            || common::pdfua1_rule_7_21_6_2_fixture("invalid_differences"),
            &["ISO 14289-1:2014:7.21.6:2"],
        ),
        (
            "pdfua1-rule-7-21-6-2-invalid-encoding.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-6-2-invalid-encoding.pdf").to_vec(),
            || common::pdfua1_rule_7_21_6_2_fixture("invalid_encoding"),
            &["ISO 14289-1:2014:7.21.6:2"],
        ),
        (
            "pdfua1-rule-7-21-6-2-matching.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-6-2-matching.pdf").to_vec(),
            || common::pdfua1_rule_7_21_6_2_fixture("matching"),
            &[],
        ),
        (
            "pdfua1-rule-7-21-6-2-missing-unicode-cmap.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-6-2-missing-unicode-cmap.pdf").to_vec(),
            || common::pdfua1_rule_7_21_6_2_fixture("missing_unicode_cmap"),
            &[],
        ),
    ],
}

#[test]
fn requires_microsoft_unicode_cmap_only_for_embedded_fonts_with_differences() {
    for (case, expected_failure_ids) in [
        ("macintosh_only_no_differences", &[][..]),
        (
            "macintosh_only_differences",
            &["ISO 14289-1:2014:7.21.6:2"][..],
        ),
        ("unembedded_macintosh_only_differences", &[][..]),
    ] {
        common::assert_pdfua1_rule_behavior(
            RULE,
            &common::pdfua1_rule_7_21_6_2_fixture(case),
            expected_failure_ids,
        );
    }
}
