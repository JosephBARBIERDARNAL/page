use page_validation::{
    PdfError, SafetyLimits, ValidationError, ValidationProfile, validate_pdf_bytes,
};

pub mod common;

#[test]
fn multiple_invalid_profiles_are_aggregated_as_one_check_failure() {
    let report = common::validate(&common::output_intent_fixture(
        "two_shared_invalid_profiles",
    ));
    common::assert_single_failure(&report, "PDFA1B-OUTPUTINTENT-001");
}

#[test]
fn normalization_retains_output_intent_diagnostics() {
    let report = common::validate(&common::output_intent_fixture("baseline"));
    let document = report.document.expect("normalized document");
    let output_intents = document.output_intents_summary;
    assert!(output_intents.present);
    assert!(output_intents.is_array);
    assert_eq!(output_intents.entries.len(), 1);
    let intent = &output_intents.entries[0];
    assert!(intent.object_id.is_some());
    assert!(intent.is_dictionary_based);
    assert!(intent.subtype_present);
    assert_eq!(intent.subtype.as_deref(), Some("GTS_PDFA1"));
    assert!(intent.dest_output_profile_present);
    assert!(intent.dest_output_profile_id.is_some());
    assert!(intent.dest_output_profile_is_stream);
    assert_eq!(
        intent.dest_output_profile_header.as_ref().map(|header| (
            header.device_class.as_str(),
            header.color_space.as_str(),
            header.version_major,
            header.version_minor,
        )),
        Some(("mntr", "RGB ", 2, 1))
    );
    assert!(intent.dest_output_profile_decode_error.is_none());
}

#[test]
fn oversized_decoded_icc_profile_is_an_operational_failure() {
    let limits = SafetyLimits {
        max_decoded_stream_size: 2048,
        ..SafetyLimits::default()
    };
    let error = validate_pdf_bytes(
        &common::output_intent_fixture("large_compressed_profile"),
        Some(ValidationProfile::PdfA1b),
        &limits,
    )
    .expect_err("ICC profile must exceed the decoded-size limit");
    assert!(matches!(
        error,
        ValidationError::Pdf(PdfError::IccDecodeLimit(_))
    ));
}
