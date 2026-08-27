use page_validation::{SafetyLimits, ValidationError, ValidationProfile, validate_bytes};

pub mod common;

const CASES: &[(&str, &[&str])] = &[
    ("no_output_intents", &[]),
    ("wrong_type_array", &[]),
    ("empty_array", &[]),
    ("non_dictionary_entries", &[]),
    ("direct_intent_dictionary", &[]),
    ("missing_s", &[]),
    ("wrong_s", &[]),
    ("missing_dest_output_profile", &[]),
    ("direct_wrong_type_profile", &[]),
    ("indirect_wrong_type_profile", &[]),
    ("truncated_profile", &["PDFA1B-OUTPUTINTENT-001"]),
    ("class_prtr", &[]),
    ("class_scnr", &["PDFA1B-OUTPUTINTENT-001"]),
    ("color_cmyk", &[]),
    ("color_gray", &[]),
    ("color_lab", &["PDFA1B-OUTPUTINTENT-001"]),
    ("version_2_15", &[]),
    ("version_3", &["PDFA1B-OUTPUTINTENT-001"]),
    ("two_shared_indirect_profiles", &[]),
    ("two_shared_invalid_profiles", &["PDFA1B-OUTPUTINTENT-001"]),
    (
        "two_identical_indirect_profiles",
        &["PDFA1B-OUTPUTINTENT-IDENTITY-001"],
    ),
    (
        "two_different_indirect_profiles",
        &["PDFA1B-OUTPUTINTENT-IDENTITY-001"],
    ),
    ("one_profile_one_missing", &[]),
    ("two_same_wrong_type_indirect_profiles", &[]),
    (
        "two_different_wrong_type_indirect_profiles",
        &["PDFA1B-OUTPUTINTENT-IDENTITY-001"],
    ),
];

#[test]
fn output_intent_cases_have_the_complete_expected_failure_delta() {
    common::assert_case_deltas(common::output_intent_fixture, "baseline", CASES);
}

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
    let error = validate_bytes(
        &common::output_intent_fixture("large_compressed_profile"),
        Some(ValidationProfile::PdfA1b),
        &limits,
    )
    .expect_err("ICC profile must exceed the decoded-size limit");
    assert!(matches!(error, ValidationError::Pdf(_)));
}
