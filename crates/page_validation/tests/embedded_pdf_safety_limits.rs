use page_validation::{
    PageError, PdfError, SafetyLimits, ValidationOptions, ValidationProfile, validate_pdf_bytes,
};

pub mod common;

const EMBEDDED_PDF_A_CONFORMANCE: &str = "PDFA2B-EMBEDDED-FILE-PDFA-001";

#[test]
fn embedded_pdf_decode_limit_is_an_operational_failure() {
    let limits = SafetyLimits::default().max_decoded_stream_size(4096);
    let error = validate_pdf_bytes(
        &common::pdfa_2_3_fixture("embedded_pdf_decode_limit"),
        &ValidationOptions::default()
            .profile(ValidationProfile::PdfA2b)
            .limits(limits),
    )
    .expect_err("the embedded PDF must exceed the decoded stream limit");

    assert!(matches!(
        error,
        PageError::Pdf(PdfError::ContentDecodeLimit(4096))
    ));
}

#[test]
fn embedded_pdf_object_limit_is_an_operational_failure() {
    let limits = SafetyLimits::default().max_object_count(20);
    let error = validate_pdf_bytes(
        &common::pdfa_2_3_fixture("embedded_pdf_object_limit"),
        &ValidationOptions::default()
            .profile(ValidationProfile::PdfA2b)
            .limits(limits),
    )
    .expect_err("the embedded PDF must exceed the object-count limit");

    assert!(matches!(
        error,
        PageError::Pdf(PdfError::TooManyObjects { limit: 20, .. })
    ));
}

#[test]
fn malformed_embedded_pdf_remains_a_conformance_failure() {
    let report = validate_pdf_bytes(
        &common::pdfa_2_3_fixture("file_spec_association"),
        &ValidationOptions::default().profile(ValidationProfile::PdfA2b),
    )
    .expect("malformed attachments remain rule failures");

    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.rule_id == EMBEDDED_PDF_A_CONFORMANCE),
        "the embedded PDF conformance rule must fail: {report}"
    );
}

#[test]
fn noncompliant_embedded_pdf_remains_a_conformance_failure() {
    let report = validate_pdf_bytes(
        &common::pdfa_2_3_fixture("embedded_pdf_noncompliant"),
        &ValidationOptions::default().profile(ValidationProfile::PdfA2b),
    )
    .expect("noncompliant attachments remain rule failures");

    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.rule_id == EMBEDDED_PDF_A_CONFORMANCE),
        "the embedded PDF conformance rule must fail: {report}"
    );
}
