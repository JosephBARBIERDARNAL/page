#[allow(dead_code)]
mod common;

use std::collections::BTreeSet;

use pdf_validation::{SafetyLimits, ValidationProfile, validate_bytes};

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
    let baseline = failure_ids(&common::output_intent_fixture("baseline"));
    for (case, expected_added) in CASES {
        let actual = failure_ids(&common::output_intent_fixture(case));
        let added = actual
            .difference(&baseline)
            .cloned()
            .collect::<BTreeSet<_>>();
        let removed = baseline
            .difference(&actual)
            .cloned()
            .collect::<BTreeSet<_>>();
        let expected_added = expected_added
            .iter()
            .map(|rule_id| (*rule_id).to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(added, expected_added, "{case}: unexpected added failures");
        assert!(
            removed.is_empty(),
            "{case}: removed baseline failures {removed:?}"
        );
    }
}

#[test]
fn multiple_invalid_profiles_are_aggregated_as_one_check_failure() {
    let report = validate_bytes(
        &common::output_intent_fixture("two_shared_invalid_profiles"),
        ValidationProfile::PdfA1b,
        &SafetyLimits::default(),
    );
    assert_eq!(
        report
            .failures
            .iter()
            .filter(|failure| failure.rule_id == "PDFA1B-OUTPUTINTENT-001")
            .count(),
        1
    );
    assert_eq!(report.failures.len(), 1, "{:#?}", report.failures);
    assert_eq!(report.checks.failed, 1);
    assert_eq!(report.checks.passed, 18);
    assert_eq!(report.checks.total, 19);
}

#[test]
fn normalization_retains_output_intent_diagnostics() {
    let report = validate_bytes(
        &common::output_intent_fixture("baseline"),
        ValidationProfile::PdfA1b,
        &SafetyLimits::default(),
    );
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
    let report = validate_bytes(
        &common::output_intent_fixture("large_compressed_profile"),
        ValidationProfile::PdfA1b,
        &limits,
    );
    assert_eq!(report.exit_code(), 1);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].rule_id, "RESOURCE-LIMIT-001");
}

fn failure_ids(bytes: &[u8]) -> BTreeSet<String> {
    validate_bytes(bytes, ValidationProfile::PdfA1b, &SafetyLimits::default())
        .failures
        .into_iter()
        .map(|failure| failure.rule_id.to_owned())
        .collect()
}
