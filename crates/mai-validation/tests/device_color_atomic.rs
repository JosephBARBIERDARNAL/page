use std::collections::BTreeSet;

use mai_validation::{SafetyLimits, ValidationProfile, validate_bytes};

#[allow(dead_code)]
mod common;

const CASES: &[(&str, &[&str])] = &[
    ("rgb_operator", &[]),
    ("cmyk_operator", &["PDFA1B-DEVICE-CMYK-001"]),
    ("gray_operator", &[]),
    ("rgb_with_cmyk_output", &["PDFA1B-DEVICE-RGB-001"]),
    ("cmyk_with_cmyk_output", &[]),
    ("gray_with_cmyk_output", &[]),
    ("rgb_without_output", &["PDFA1B-DEVICE-RGB-001"]),
    ("cmyk_without_output", &["PDFA1B-DEVICE-CMYK-001"]),
    ("gray_without_output", &["PDFA1B-DEVICE-GRAY-001"]),
    ("rgb_wrong_s", &["PDFA1B-DEVICE-RGB-001"]),
    ("explicit_rgb", &["PDFA1B-DEVICE-RGB-001"]),
    ("resource_rgb", &["PDFA1B-DEVICE-RGB-001"]),
    ("unused_resource_rgb", &[]),
    ("default_rgb_override", &[]),
    ("form_rgb", &["PDFA1B-DEVICE-RGB-001"]),
    ("image_rgb", &["PDFA1B-DEVICE-RGB-001"]),
    ("inline_rgb", &["PDFA1B-DEVICE-RGB-001"]),
    ("shading_rgb", &["PDFA1B-DEVICE-RGB-001"]),
    ("indexed_rgb", &["PDFA1B-DEVICE-RGB-001"]),
    ("separation_rgb", &["PDFA1B-DEVICE-RGB-001"]),
    ("devicen_rgb", &["PDFA1B-DEVICE-RGB-001"]),
    ("pattern_rgb", &[]),
];

#[test]
fn device_color_cases_have_the_complete_expected_failure_delta() {
    let baseline = failure_ids(&common::device_color_fixture("baseline"));
    for (case, expected_added) in CASES {
        let actual = failure_ids(&common::device_color_fixture(case));
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
fn device_color_failure_reports_the_selected_space_and_output_space() {
    let report = validate_bytes(
        &common::device_color_fixture("rgb_with_cmyk_output"),
        ValidationProfile::PdfA1b,
        &SafetyLimits::default(),
    );
    let failure = report
        .failures
        .iter()
        .find(|failure| failure.rule_id == "PDFA1B-DEVICE-RGB-001")
        .expect("DeviceRGB failure");
    assert!(failure.message.contains("DeviceRGB"));
    assert!(failure.message.contains("CMYK"));
    assert_eq!(report.checks.total, 101);
    assert_eq!(report.checks.failed, 1);
    assert_eq!(report.checks.passed, 100);
}

fn failure_ids(bytes: &[u8]) -> BTreeSet<String> {
    validate_bytes(bytes, ValidationProfile::PdfA1b, &SafetyLimits::default())
        .failures
        .into_iter()
        .map(|failure| failure.rule_id.to_owned())
        .collect()
}
