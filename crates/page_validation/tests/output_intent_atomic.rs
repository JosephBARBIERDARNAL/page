use page_validation::{
    PdfError, SafetyLimits, ValidationError, ValidationOptions, ValidationProfile,
    validate_pdf_bytes,
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
    let value = serde_json::to_value(&report).expect("serialize report");
    let output_intents = &value["document"]["output_intents_summary"];
    assert_eq!(output_intents["present"], true);
    assert_eq!(output_intents["is_array"], true);
    assert_eq!(output_intents["entries"].as_array().unwrap().len(), 1);
    let intent = &output_intents["entries"][0];
    assert!(intent["object_id"].is_object());
    assert_eq!(intent["is_dictionary_based"], true);
    assert_eq!(intent["subtype_present"], true);
    assert_eq!(intent["subtype"], "GTS_PDFA1");
    assert_eq!(intent["dest_output_profile_present"], true);
    assert!(intent["dest_output_profile_id"].is_object());
    assert_eq!(intent["dest_output_profile_is_stream"], true);
    assert_eq!(
        intent["dest_output_profile_header"],
        serde_json::json!({
            "device_class": "mntr",
            "color_space": "RGB ",
            "version_major": 2,
            "version_minor": 1,
        })
    );
    assert!(intent["dest_output_profile_decode_error"].is_null());
}

#[test]
fn oversized_decoded_icc_profile_is_an_operational_failure() {
    let limits = SafetyLimits::default().max_decoded_stream_size(2048);
    let error = validate_pdf_bytes(
        &common::output_intent_fixture("large_compressed_profile"),
        &ValidationOptions::default()
            .profile(ValidationProfile::PdfA1b)
            .limits(limits),
    )
    .expect_err("ICC profile must exceed the decoded-size limit");
    assert!(matches!(
        error,
        ValidationError::Pdf(PdfError::IccDecodeLimit(_))
    ));
}
