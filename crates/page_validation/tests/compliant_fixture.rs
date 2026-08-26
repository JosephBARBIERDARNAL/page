use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

#[test]
fn typst_pdfa_1b_fixture_passes_all_implemented_checks() {
    let report = validate_bytes_with_profile(
        include_bytes!("fixtures/typst-pdfa-1b.pdf"),
        ValidationProfile::PdfA1b,
        &SafetyLimits::default(),
    );

    assert!(report.checks_passed, "{report}");
    assert!(report.failures.is_empty(), "{report}");
    let total = ValidationProfile::PdfA1b.implemented_check_count();
    assert_eq!(report.checks.total, total);
    assert_eq!(report.checks.passed, total);
    assert_eq!(report.checks.failed, 0);

    let document = report.document.expect("parsed PDF document");
    assert_eq!(document.version, "1.4");
    assert_eq!(document.page_count, 1);
}
