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
    (
        "devicen_nine_components",
        &["PDFA1B-DEVICE-RGB-001", "PDFA1B-DEVICEN-COMPONENTS-001"],
    ),
    ("pattern_rgb", &[]),
];

#[test]
fn device_color_cases_have_the_complete_expected_failure_delta() {
    common::assert_case_deltas(common::device_color_fixture, "baseline", CASES);
}

#[test]
fn device_color_failure_reports_the_selected_space_and_output_space() {
    let report = common::validate(&common::device_color_fixture("rgb_with_cmyk_output"));
    let failure = common::assert_single_failure(&report, "PDFA1B-DEVICE-RGB-001");
    assert!(failure.message.contains("DeviceRGB"));
    assert!(failure.message.contains("CMYK"));
}
