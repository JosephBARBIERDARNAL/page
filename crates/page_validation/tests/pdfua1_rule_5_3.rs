use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-ID-PART-PREFIX-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:5:3";

#[test]
fn pdfua1_rule_5_3_fixtures_require_pdfuaid_part_prefix() {
    let canonical_prefix = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-5-3-canonical-prefix.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(canonical_prefix.checks_passed, "{canonical_prefix}");
    assert_eq!(canonical_prefix.checks.total, 27);
    assert_eq!(canonical_prefix.checks.passed, 27);
    assert!(canonical_prefix.failures.is_empty());

    let wrong_prefix = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-5-3-wrong-prefix.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!wrong_prefix.checks_passed, "{wrong_prefix}");
    assert_eq!(wrong_prefix.checks.total, 27);
    assert_eq!(wrong_prefix.checks.failed, 1);
    assert_eq!(wrong_prefix.failures.len(), 1);
    assert_eq!(wrong_prefix.failures[0].rule_id, RULE);
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 5-3 fixtures"]
fn regenerate_pdfua1_rule_5_3_fixtures() {
    fs::write(
        "tests/fixtures/pdfua1-rule-5-3-canonical-prefix.pdf",
        common::pdfua1_rule_5_3_fixture("canonical_prefix"),
    )
    .expect("write PDF/UA-1 rule 5-3 pass fixture");
    fs::write(
        "tests/fixtures/pdfua1-rule-5-3-wrong-prefix.pdf",
        common::pdfua1_rule_5_3_fixture("wrong_prefix"),
    )
    .expect("write PDF/UA-1 rule 5-3 fail fixture");
}

#[test]
fn pdfua1_rule_5_3_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (fixture, should_fail) in [
        ("pdfua1-rule-5-3-canonical-prefix.pdf", false),
        ("pdfua1-rule-5-3-wrong-prefix.pdf", true),
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
