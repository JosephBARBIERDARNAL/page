#![expect(
    clippy::panic,
    reason = "fixture dispatch deliberately fails loudly for an undeclared test case"
)]

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{
    ComparisonClassification, DifferentialRunner, ReferenceConfig, ReferenceProfile,
};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-TRUETYPE-NONSYMBOLIC-ENCODING-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.6:2";

#[test]
fn pdfua1_rule_7_21_6_2_requires_valid_nonsymbolic_truetype_encoding() {
    let matching = validate_bytes_with_profile(
        fixture_bytes("matching"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(matching.checks_passed, "{matching}");
    assert!(matching.failures.is_empty(), "{matching}");

    for fixture in [
        "invalid_encoding",
        "invalid_differences",
        "missing_unicode_cmap",
    ] {
        let report = validate_bytes_with_profile(
            fixture_bytes(fixture),
            ValidationProfile::PdfUa1,
            &SafetyLimits::default(),
        );
        assert!(!report.checks_passed, "{fixture}: {report}");
        assert_eq!(report.checks.failed, 1, "{fixture}: {report}");
        assert_eq!(report.failures.len(), 1, "{fixture}: {report}");
        assert_eq!(report.failures[0].rule_id, RULE, "{fixture}: {report}");
    }
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.21.6-2 fixtures"]
fn regenerate_pdfua1_rule_7_21_6_2_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-21-6-2-matching.pdf", "matching"),
        (
            "pdfua1-rule-7-21-6-2-invalid-encoding.pdf",
            "invalid_encoding",
        ),
        (
            "pdfua1-rule-7-21-6-2-invalid-differences.pdf",
            "invalid_differences",
        ),
        (
            "pdfua1-rule-7-21-6-2-missing-unicode-cmap.pdf",
            "missing_unicode_cmap",
        ),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_21_6_2_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.21.6-2 fixture");
    }
}

#[test]
fn pdfua1_rule_7_21_6_2_fixtures_match_verapdf_1302_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-21-6-2-matching.pdf", false),
        ("pdfua1-rule-7-21-6-2-invalid-encoding.pdf", true),
        ("pdfua1-rule-7-21-6-2-invalid-differences.pdf", true),
        // veraPDF 1.30.2 accepts this normative failure; see the assertion
        // below for the intentional local/reference discrepancy.
        ("pdfua1-rule-7-21-6-2-missing-unicode-cmap.pdf", false),
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture);
        let report = runner.compare_file(&path, &SafetyLimits::default());
        let failed = report
            .reference_result
            .as_ref()
            .expect("veraPDF result")
            .failed_rule_ids
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            failed.contains(REFERENCE_RULE),
            should_fail,
            "{fixture}: {report}"
        );
        if fixture.ends_with("missing-unicode-cmap.pdf") {
            // veraPDF 1.30.2's executable predicate omits the Microsoft
            // Unicode cmap condition present in the written requirement, so
            // this normative local failure is an intentional discrepancy.
            assert_eq!(
                report.classification,
                ComparisonClassification::LocalFalseNegative,
                "{fixture}: {report}"
            );
        }
        assert!(report.operational_failure.is_none(), "{fixture}: {report}");
    }
}

fn fixture_bytes(fixture: &str) -> &'static [u8] {
    match fixture {
        "matching" => include_bytes!("fixtures/pdfua1-rule-7-21-6-2-matching.pdf"),
        "invalid_encoding" => {
            include_bytes!("fixtures/pdfua1-rule-7-21-6-2-invalid-encoding.pdf")
        }
        "invalid_differences" => {
            include_bytes!("fixtures/pdfua1-rule-7-21-6-2-invalid-differences.pdf")
        }
        "missing_unicode_cmap" => {
            include_bytes!("fixtures/pdfua1-rule-7-21-6-2-missing-unicode-cmap.pdf")
        }
        _ => panic!("unknown PDF/UA-1 rule 7.21.6-2 fixture case {fixture}"),
    }
}
