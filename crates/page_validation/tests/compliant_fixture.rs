use page_validation::{
    PdfError, SafetyLimits, ValidationError, ValidationProfile, is_pdf_compliant, validate_pdf,
    validate_pdf_bytes,
};

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
    assert_eq!(report.rules.total, total);
    assert_eq!(report.rules.passed, total);
    assert_eq!(report.rules.failed, 0);

    let document = report.document.expect("parsed PDF document");
    assert_eq!(document.version, "1.4");
    assert_eq!(document.page_count, 1);
}

#[test]
fn unlimited_file_input_works_with_detailed_and_fast_validation() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/typst-pdfa-1b.pdf");
    let profile = Some(ValidationProfile::PdfA1b);
    let bounded = SafetyLimits {
        max_input_size: 1,
        ..SafetyLimits::unlimited()
    };
    for error in [
        validate_pdf(&path, profile, &bounded).expect_err("detailed input bound"),
        is_pdf_compliant(&path, profile, &bounded).expect_err("fast input bound"),
    ] {
        assert!(matches!(
            error,
            ValidationError::Pdf(PdfError::InputTooLarge { limit: 1, .. })
        ));
    }
    let unlimited = SafetyLimits::unlimited();
    assert!(
        validate_pdf(&path, profile, &unlimited)
            .expect("unlimited file read")
            .is_compliant
    );
    assert!(is_pdf_compliant(&path, profile, &unlimited).expect("fast unlimited file read"));
}
