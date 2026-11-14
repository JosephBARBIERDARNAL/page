pub mod common;

const RULE: &str = "ISO 14289-1:2014:7.2:42";

#[test]
fn reports_column_overflow_in_preference_to_missing_columns() {
    for (case, expected_overflow, expected_missing) in [
        ("short_then_wide", true, false),
        ("short_only", false, true),
        ("wide_only", true, false),
        ("rectangular", false, false),
    ] {
        let report = page_validation::validate_pdf_bytes(
            &common::pdfua1_rule_7_2_42_fixture(case),
            &page_validation::ValidationOptions::default()
                .profile(page_validation::ValidationProfile::PdfUa1),
        )
        .expect("validate table columns");
        for (rule, expected) in [
            (RULE, expected_overflow),
            ("ISO 14289-1:2014:7.2:43", expected_missing),
            ("ISO 14289-1:2014:7.2:41", false),
        ] {
            assert_eq!(
                report
                    .failures
                    .iter()
                    .any(|failure| failure.rule_id == rule),
                expected,
                "{case}, {rule}: {report:#?}"
            );
        }
        if expected_overflow || expected_missing {
            assert!(!report.is_compliant, "{case}");
        }
    }
}

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-2-42-allowed.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-42-allowed.pdf").to_vec(),
            || common::pdfua1_rule_7_2_42_fixture("allowed"),
            &[],
        ),
        (
            "pdfua1-rule-7-2-42-invalid.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-2-42-invalid.pdf").to_vec(),
            || common::pdfua1_rule_7_2_42_fixture("invalid"),
            &["ISO 14289-1:2014:7.2:42"],
        ),
    ],
}
