use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

use std::{env, path::Path};

use page_validation::differential::{
    ComparisonClassification, DifferentialRunner, ReferenceConfig, ReferenceProfile,
};

#[test]
fn canonical_pdfa_1a_is_locally_compliant() {
    let report = validate_bytes_with_profile(
        include_bytes!("fixtures/canonical-pdfa-1a.pdf"),
        ValidationProfile::PdfA1a,
        &SafetyLimits::default(),
    );

    assert!(report.checks_passed, "{report}");
    assert!(report.failures.is_empty(), "{report}");
}

#[test]
fn canonical_pdfa_1b_is_locally_compliant() {
    let report = validate_bytes_with_profile(
        include_bytes!("fixtures/canonical-pdfa-1b.pdf"),
        ValidationProfile::PdfA1b,
        &SafetyLimits::default(),
    );

    assert!(report.checks_passed, "{report}");
    assert!(report.failures.is_empty(), "{report}");
}

#[test]
fn canonical_pdfa_1a_matches_verapdf() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        eprintln!("VERAPDF_BIN is unset; skipping opt-in veraPDF canonical test");
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfA1a;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    let report = runner.compare_file(
        Path::new("tests/fixtures/canonical-pdfa-1a.pdf"),
        &SafetyLimits::default(),
    );

    assert_eq!(
        report.classification,
        ComparisonClassification::Agreement,
        "{report}"
    );
    assert!(report.local_report.failures.is_empty(), "{report}");
    assert_eq!(
        report
            .reference_result
            .as_ref()
            .expect("veraPDF result")
            .compliant,
        Some(true),
        "{report}"
    );
}
