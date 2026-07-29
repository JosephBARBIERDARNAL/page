use std::collections::BTreeSet;

use mai_validation::{SafetyLimits, ValidationProfile, validate_bytes};

#[allow(dead_code)]
mod common;

const CASES: &[(&str, &[&str])] = &[
    ("class_prtr", &[]),
    ("class_mntr", &[]),
    ("class_scnr", &[]),
    ("class_spac", &[]),
    ("color_rgb", &[]),
    ("color_cmyk", &[]),
    ("color_gray", &[]),
    ("color_lab", &[]),
    ("version_2_15", &[]),
    ("invalid_class", &["PDFA1B-ICCBASED-001"]),
    (
        "invalid_color_space",
        &["PDFA1B-ICCBASED-001", "PDFA1B-ICCBASED-COMPONENTS-001"],
    ),
    ("version_3", &["PDFA1B-ICCBASED-001"]),
    (
        "truncated_profile",
        &["PDFA1B-ICCBASED-001", "PDFA1B-ICCBASED-COMPONENTS-001"],
    ),
    (
        "undecodable_profile",
        &["PDFA1B-ICCBASED-001", "PDFA1B-ICCBASED-COMPONENTS-001"],
    ),
    ("missing_n", &["PDFA1B-ICCBASED-COMPONENTS-001"]),
    ("wrong_n", &["PDFA1B-ICCBASED-COMPONENTS-001"]),
    ("non_integer_n", &["PDFA1B-ICCBASED-COMPONENTS-001"]),
    ("unused_resource", &[]),
    ("default_gray", &["PDFA1B-ICCBASED-001"]),
    ("default_rgb", &["PDFA1B-ICCBASED-001"]),
    ("default_cmyk", &["PDFA1B-ICCBASED-001"]),
    ("unused_default", &[]),
    ("inherited_resources", &[]),
    ("missing_profile", &[]),
    ("wrong_profile_type", &[]),
    ("form_used", &["PDFA1B-ICCBASED-001"]),
    ("form_unused_resource", &[]),
    ("form_unreferenced", &[]),
    ("form_parent_fallback", &["PDFA1B-ICCBASED-001"]),
    ("nested_form_page_fallback", &["PDFA1B-ICCBASED-001"]),
    ("nested_form_used", &["PDFA1B-ICCBASED-001"]),
    ("cyclic_form", &["PDFA1B-ICCBASED-001"]),
    ("image_used", &["PDFA1B-ICCBASED-001"]),
    ("image_unused_resource", &[]),
    ("image_unreferenced", &[]),
    ("image_mask_ignores_color_space", &[]),
    ("image_smask_used", &["PDFA1B-XOBJECT-SMASK-001"]),
    ("image_mask_image_used", &["PDFA1B-IMAGE-MASK-BPC-001"]),
    (
        "image_alternate_used",
        &["PDFA1B-ICCBASED-001", "PDFA1B-IMAGE-ALTERNATES-001"],
    ),
    ("inline_image_used", &["PDFA1B-ICCBASED-001"]),
    ("shading_used", &["PDFA1B-ICCBASED-001"]),
    ("indexed_base_used", &["PDFA1B-ICCBASED-001"]),
    ("repeated_shared_valid", &[]),
    ("repeated_shared_invalid", &["PDFA1B-ICCBASED-001"]),
    (
        "two_invalid_profiles",
        &["PDFA1B-ICCBASED-001", "PDFA1B-ICCBASED-COMPONENTS-001"],
    ),
];

#[test]
fn icc_based_cases_have_the_complete_expected_failure_delta() {
    let baseline = failure_ids(&common::icc_based_fixture("baseline"));
    for (case, expected_added) in CASES {
        let actual = failure_ids(&common::icc_based_fixture(case));
        let (added, removed) = common::rule_delta(&baseline, &actual);
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
fn shared_invalid_profile_is_reported_once_with_its_object_id() {
    let report = validate_bytes(
        &common::icc_based_fixture("repeated_shared_invalid"),
        ValidationProfile::PdfA1b,
        &SafetyLimits::default(),
    );
    let failures = report
        .failures
        .iter()
        .filter(|failure| failure.rule_id == "PDFA1B-ICCBASED-001")
        .collect::<Vec<_>>();
    assert_eq!(failures.len(), 1);
    assert!(failures[0].object_id.is_some());
    assert_eq!(report.checks.total, 99);
    assert_eq!(report.checks.failed, 1);
    assert_eq!(report.checks.passed, 98);
}

#[test]
fn component_mismatch_is_reported_with_its_profile_object_id() {
    let report = validate_bytes(
        &common::icc_based_fixture("wrong_n"),
        ValidationProfile::PdfA1b,
        &SafetyLimits::default(),
    );
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
    let report = validate_bytes(
        &common::icc_based_fixture("two_invalid_profiles"),
        ValidationProfile::PdfA1b,
        &SafetyLimits::default(),
    );
    let failures = report
        .failures
        .iter()
        .filter(|failure| failure.rule_id == "PDFA1B-ICCBASED-001")
        .collect::<Vec<_>>();
    assert_eq!(failures.len(), 1);
    assert!(failures[0].object_id.is_none());
    assert!(failures[0].message.contains("; "));
}

#[test]
fn oversized_decoded_icc_based_profile_is_an_operational_failure() {
    let limits = SafetyLimits {
        max_decoded_stream_size: 2048,
        ..SafetyLimits::default()
    };
    let report = validate_bytes(
        &common::icc_based_fixture("large_compressed_profile"),
        ValidationProfile::PdfA1b,
        &limits,
    );
    assert_eq!(report.exit_code(), 1);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].rule_id, "RESOURCE-LIMIT-001");
}

#[test]
fn cyclic_and_deep_composite_color_spaces_hit_the_reference_depth_limit() {
    let limits = SafetyLimits {
        max_reference_depth: 4,
        ..SafetyLimits::default()
    };
    for case in ["cyclic_indexed", "deep_indexed"] {
        let report = validate_bytes(
            &common::icc_based_fixture(case),
            ValidationProfile::PdfA1b,
            &limits,
        );
        assert_eq!(report.exit_code(), 1, "{case}: {report:#?}");
        assert_eq!(report.failures.len(), 1, "{case}: {report:#?}");
        assert_eq!(report.failures[0].rule_id, "RESOURCE-LIMIT-001", "{case}");
    }
}

fn failure_ids(bytes: &[u8]) -> BTreeSet<String> {
    validate_bytes(bytes, ValidationProfile::PdfA1b, &SafetyLimits::default())
        .failures
        .into_iter()
        .map(|failure| failure.rule_id.to_owned())
        .collect()
}
