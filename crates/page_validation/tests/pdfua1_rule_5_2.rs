use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFUA1-ID-PART-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:5:2";

#[test]
fn pdfua1_rule_5_2_fixtures_require_pdfua_part_one() {
    let present = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-5-2-present.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(present.checks_passed, "{present}");
    assert_eq!(present.checks.total, 15);
    assert_eq!(present.checks.passed, 15);
    assert!(present.failures.is_empty());

    let wrong_part = validate_bytes_with_profile(
        include_bytes!("fixtures/pdfua1-rule-5-2-wrong-part.pdf"),
        ValidationProfile::PdfUa1,
        &SafetyLimits::default(),
    );
    assert!(!wrong_part.checks_passed, "{wrong_part}");
    assert_eq!(wrong_part.checks.total, 15);
    assert_eq!(wrong_part.checks.failed, 1);
    assert_eq!(wrong_part.failures.len(), 1);
    assert_eq!(wrong_part.failures[0].rule_id, RULE);
}

#[test]
#[ignore = "maintenance generator for PDF/UA-1 rule 5-2 fixtures"]
fn regenerate_pdfua1_rule_5_2_fixtures() {
    fs::write(
        "tests/fixtures/pdfua1-rule-5-2-present.pdf",
        common::pdfua1_rule_5_2_fixture("part_one"),
    )
    .expect("write PDF/UA-1 rule 5-2 pass fixture");
    fs::write(
        "tests/fixtures/pdfua1-rule-5-2-wrong-part.pdf",
        common::pdfua1_rule_5_2_fixture("part_two"),
    )
    .expect("write PDF/UA-1 rule 5-2 fail fixture");
}

#[test]
fn pdfua1_rule_5_2_fixtures_match_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfUa1;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    for (fixture, should_fail) in [
        ("pdfua1-rule-5-2-present.pdf", false),
        ("pdfua1-rule-5-2-wrong-part.pdf", true),
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
