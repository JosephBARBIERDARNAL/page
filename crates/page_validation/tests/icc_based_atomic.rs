use page_validation::{
    PdfError, SafetyLimits, ValidationError, ValidationProfile, validate_pdf_bytes,
};

pub mod common;

#[test]
fn shared_invalid_profile_is_reported_once_with_its_object_id() {
    let report = common::validate(&common::icc_based_fixture("repeated_shared_invalid"));
    let failure = common::assert_single_failure(&report, "PDFA1B-ICCBASED-001");
    assert!(failure.object_id.is_some());
}

#[test]
fn component_mismatch_is_reported_with_its_profile_object_id() {
    let report = common::validate(&common::icc_based_fixture("wrong_n"));
    let failures = report
        .failures
        .iter()
        .filter(|failure| failure.rule_id == "PDFA1B-ICCBASED-COMPONENTS-001")
        .collect::<Vec<_>>();
    assert_eq!(failures.len(), 1);
    assert!(failures[0].object_id.is_some());
    assert!(failures[0].message.contains("/N Some(4)"));
    assert!(failures[0].message.contains("\"RGB \""));
}

#[test]
fn multiple_invalid_profiles_are_aggregated_without_an_object_id() {
    let report = common::validate(&common::icc_based_fixture("two_invalid_profiles"));
    let failures = report
        .failures
        .iter()
        .filter(|failure| failure.rule_id == "PDFA1B-ICCBASED-001")
        .collect::<Vec<_>>();
    assert_eq!(failures.len(), 1);
    assert!(failures[0].object_id.is_none());
    assert!(failures[0].message.contains("ICCBased profile"));
}

#[test]
fn oversized_decoded_icc_based_profile_is_an_operational_failure() {
    let limits = SafetyLimits {
        max_decoded_stream_size: 2048,
        ..SafetyLimits::default()
    };
    let error = validate_pdf_bytes(
        &common::icc_based_fixture("large_compressed_profile"),
        Some(ValidationProfile::PdfA1b),
        &limits,
    )
    .expect_err("ICC profile must exceed the decoded-size limit");
    assert!(matches!(
        error,
        ValidationError::Pdf(PdfError::IccDecodeLimit(_))
    ));
}

#[test]
fn cyclic_and_deep_composite_color_spaces_hit_the_reference_depth_limit() {
    let limits = SafetyLimits {
        max_reference_depth: 4,
        ..SafetyLimits::default()
    };
    for case in ["cyclic_indexed", "deep_indexed"] {
        let error = validate_pdf_bytes(
            &common::icc_based_fixture(case),
            Some(ValidationProfile::PdfA1b),
            &limits,
        )
        .expect_err("{case} must exceed the configured reference depth");
        assert!(
            matches!(error, ValidationError::Pdf(PdfError::ReferenceDepth(4))),
            "{case}: {error:?}"
        );
    }
}
