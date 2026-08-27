use page_validation::{SafetyLimits, ValidationProfile, validate_pdf_bytes};

#[test]
fn typst_pdfa_1b_fixture_passes_all_implemented_checks() {
    let report = validate_pdf_bytes(
        include_bytes!("fixtures/typst-pdfa-1b.pdf"),
        Some(ValidationProfile::PdfA1b),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");

    assert!(report.is_compliant, "{report}");
    assert!(report.failures.is_empty(), "{report}");
    let total = ValidationProfile::PdfA1b.implemented_check_count();
    assert_eq!(report.checks.total, total);
    assert_eq!(report.checks.passed, total);
    assert_eq!(report.checks.failed, 0);

    let document = report.document.expect("parsed PDF document");
    assert_eq!(document.version, "1.4");
    assert_eq!(document.page_count, 1);
}
