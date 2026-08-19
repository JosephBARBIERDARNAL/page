use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-STRUCT-TREE-ROLE-MAP-CYCLE-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.1:6";

#[test]
fn pdfua1_rule_7_1_6_rejects_circular_role_map_mappings() {
    let acyclic_mapping = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-1-6-acyclic-mapping.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(acyclic_mapping.checks_passed, "{acyclic_mapping}");
    assert_eq!(acyclic_mapping.checks.total, 31);
    assert_eq!(acyclic_mapping.checks.passed, 31);
    assert!(acyclic_mapping.failures.is_empty());

    let circular_mapping = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-1-6-circular-mapping.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!circular_mapping.checks_passed, "{circular_mapping}");
    assert_eq!(circular_mapping.checks.total, 31);
    assert_eq!(circular_mapping.checks.failed, 1);
    assert_eq!(circular_mapping.failures.len(), 1);
    assert_eq!(circular_mapping.failures[0].rule_id, RULE);
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.1-6 fixtures"]
fn regenerate_pdfua1_rule_7_1_6_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-1-6-acyclic-mapping.pdf", "acyclic_mapping"),
        ("pdfua1-rule-7-1-6-circular-mapping.pdf", "circular_mapping"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_1_6_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.1-6 fixture");
    }
}

#[test]
fn pdfua1_rule_7_1_6_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (fixture, should_fail) in [
        ("pdfua1-rule-7-1-6-acyclic-mapping.pdf", false),
        ("pdfua1-rule-7-1-6-circular-mapping.pdf", true),
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture);
        let report = runner.compare_file(&path, &SafetyLimits::default());
        let reference = report.reference_result.as_ref().expect("veraPDF result");
        let failed = reference
            .failed_rule_ids
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            failed.contains(REFERENCE_RULE),
            should_fail,
            "{fixture}: {report}"
        );
    }
}
