use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-STRUCT-ELEMENT-PARENT-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.1:12";

#[test]
fn pdfua1_rule_7_1_12_fixtures_require_structure_element_parent() {
    let present = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-1-12-present.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(present.checks_passed, "{present}");
    assert_eq!(present.checks.total, 15);
    assert_eq!(present.checks.passed, 15);
    assert!(present.failures.is_empty());

    let missing = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-7-1-12-missing.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!missing.checks_passed, "{missing}");
    assert_eq!(missing.checks.total, 15);
    assert_eq!(missing.checks.failed, 1);
    assert_eq!(missing.failures.len(), 1);
    assert_eq!(missing.failures[0].rule_id, RULE);
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 7.1-12 fixtures"]
fn regenerate_pdfua1_rule_7_1_12_fixtures() {
    for (fixture, case) in [
        ("pdfua1-rule-7-1-12-present.pdf", "present"),
        ("pdfua1-rule-7-1-12-missing.pdf", "missing"),
    ] {
        fs::write(
            Path::new("tests/fixtures").join(fixture),
            common::pdfua1_rule_7_1_12_fixture(case),
        )
        .expect("write PDF/UA-1 rule 7.1-12 fixture");
    }
}

#[test]
fn pdfua1_rule_7_1_12_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    // veraPDF 1.30.2 does not emit 7.1-12 for a missing /P; it reports the
    // related tagging failure instead, so the local rule is intentionally a
    // stricter check for this fixture.
    for (fixture, reference_should_fail_rule) in [
        ("pdfua1-rule-7-1-12-present.pdf", false),
        ("pdfua1-rule-7-1-12-missing.pdf", false),
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
            reference_should_fail_rule,
            "{fixture}: {report}"
        );
    }
}
