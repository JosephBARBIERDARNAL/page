use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-LANGUAGE-TAG-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.2:29";

#[test]
fn pdfua1_rule_7_2_29_requires_language_tags_at_all_allowed_locations() {
    for case in ["catalog_valid", "structure_valid", "property_valid"] {
        let report = validate_bytes_with_profile(
            &common::pdfua1_rule_7_2_29_fixture(case),
            ValidationProfile::PdfUa1,
            &SafetyLimits::default(),
        );
        assert!(report.checks_passed, "{case}: {report}");
        assert!(report.failures.is_empty());
    }

    for case in ["catalog_invalid", "structure_invalid", "property_invalid"] {
        let report = validate_bytes_with_profile(
            &common::pdfua1_rule_7_2_29_fixture(case),
            ValidationProfile::PdfUa1,
            &SafetyLimits::default(),
        );
        assert!(!report.checks_passed, "{case}: {report}");
        assert_eq!(report.checks.failed, 1);
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].rule_id, RULE);
    }
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.2-29 fixtures"]
fn regenerate_pdfua1_rule_7_2_29_fixtures() {
    for case in [
        "catalog_valid",
        "catalog_invalid",
        "structure_valid",
        "structure_invalid",
        "property_valid",
        "property_invalid",
    ] {
        fs::write(
            Path::new("tests/fixtures").join(format!("pdfua1-rule-7-2-29-{case}.pdf")),
            common::pdfua1_rule_7_2_29_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.2-29 fixture");
    }
}

#[test]
fn pdfua1_rule_7_2_29_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (case, should_fail) in [
        ("catalog_valid", false),
        ("catalog_invalid", true),
        ("structure_valid", false),
        ("structure_invalid", true),
        ("property_valid", false),
        ("property_invalid", true),
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(format!("pdfua1-rule-7-2-29-{case}.pdf"));
        let report = runner.compare_file(&path, &SafetyLimits::default());
        let failed = report
            .reference_result
            .as_ref()
            .unwrap_or_else(|| panic!("{report}"))
            .failed_rule_ids
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            failed.contains(REFERENCE_RULE),
            should_fail,
            "{case}: {report}"
        );
    }
}
