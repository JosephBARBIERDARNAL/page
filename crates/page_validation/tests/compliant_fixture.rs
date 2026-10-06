use page_validation::{
    PageError, PdfError, SafetyLimits, ValidationOptions, ValidationProfile, is_pdf_compliant,
    validate_pdf, validate_pdf_bytes,
};

#[test]
fn typst_pdfa_1b_fixture_passes_all_implemented_checks() {
    let report = validate_pdf_bytes(
        include_bytes!("fixtures/typst-pdfa-1b.pdf"),
        &ValidationOptions::default().profile(ValidationProfile::PdfA1b),
    )
    .expect("explicit profile validation");

    assert!(report.is_compliant, "{report}");
    assert!(report.failures.is_empty(), "{report}");
    let total = ValidationProfile::PdfA1b.implemented_check_count();
    assert_eq!(report.rules.total, total);
    assert_eq!(report.rules.passed, total);
    assert_eq!(report.rules.failed, 0);

    let document = report.document;
    assert_eq!(document.version, "1.4");
    assert!(!document.encrypted);
    assert_eq!(document.page_count, 1);
    assert!(document.object_count > 0);
}

#[test]
fn unlimited_file_input_works_with_detailed_and_lazy_validation() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/typst-pdfa-1b.pdf");
    let options = ValidationOptions::default().profile(ValidationProfile::PdfA1b);
    let bounded = options
        .clone()
        .limits(SafetyLimits::unlimited().with_max_input_size(1));
    for error in [
        validate_pdf(&path, &bounded).expect_err("detailed input bound"),
        is_pdf_compliant(&path, &bounded).expect_err("lazy input bound"),
    ] {
        assert!(matches!(
            error,
            PageError::Pdf(PdfError::InputTooLarge { limit: 1, .. })
        ));
    }
    let unlimited = options.limits(SafetyLimits::unlimited());
    assert!(
        validate_pdf(&path, &unlimited)
            .expect("unlimited file read")
            .is_compliant
    );
    assert!(is_pdf_compliant(&path, &unlimited).expect("lazy unlimited file read"));
}
