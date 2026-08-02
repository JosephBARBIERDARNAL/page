#[allow(dead_code)]
mod common;

const ICC: &str = "PDFA1B-ICCBASED-001";
const RGB: &str = "PDFA1B-DEVICE-RGB-001";
const CMYK: &str = "PDFA1B-DEVICE-CMYK-001";
const GRAY: &str = "PDFA1B-DEVICE-GRAY-001";
const INTENT: &str = "PDFA1B-RENDERING-INTENT-001";
const GROUP: &str = "PDFA1B-TRANSPARENCY-GROUP-001";
const ALTERNATES: &str = "PDFA1B-IMAGE-ALTERNATES-001";
const OUTPUT: &str = "PDFA1B-OUTPUTINTENT-001";
const IDENTITY: &str = "PDFA1B-OUTPUTINTENT-IDENTITY-001";

const CASES: &[(&str, &[&str])] = &[
    ("icc_separation_alternate", &[ICC]),
    ("icc_separation_valid", &[]),
    ("icc_devicen_alternate", &[ICC]),
    ("icc_devicen_valid", &[]),
    ("icc_devicen_wrong_n", &["PDFA1B-ICCBASED-COMPONENTS-001"]),
    ("icc_page_group", &[ICC, GROUP]),
    ("icc_page_group_valid", &[GROUP]),
    ("icc_page_group_wrong_type", &[GROUP]),
    ("icc_page_group_inherited", &[ICC, GROUP]),
    ("icc_form_group", &[ICC, GROUP]),
    ("icc_soft_mask_group", &["PDFA1B-EXTGSTATE-SMASK-001"]),
    ("icc_annotation_appearance", &[ICC]),
    ("icc_annotation_appearance_valid", &[]),
    ("icc_annotation_state", &[ICC]),
    ("icc_annotation_unreferenced", &[]),
    ("icc_pattern_content", &[ICC]),
    ("icc_pattern_content_valid", &[]),
    ("icc_pattern_unused", &[]),
    ("icc_pattern_without_selection", &[]),
    ("icc_pattern_underlying", &[]),
    ("icc_shading_pattern_direct", &[ICC]),
    ("icc_shading_pattern_indirect", &[ICC]),
    ("icc_shading_pattern_unused", &[]),
    ("device_page_group", &[RGB, GROUP]),
    ("device_form_group", &[RGB, GROUP]),
    ("device_soft_mask_group", &["PDFA1B-EXTGSTATE-SMASK-001"]),
    ("device_annotation_appearance", &[RGB]),
    ("device_annotation_unreferenced", &[]),
    ("device_pattern_content", &[RGB]),
    ("device_pattern_unused", &[]),
    ("device_image_default", &[]),
    ("device_image_inherited_default", &[]),
    ("device_image_default_wrong_type", &[]),
    ("device_inline_default", &[]),
    ("device_indexed_default", &[]),
    ("device_separation_default", &[]),
    ("device_devicen_default", &[]),
    ("device_output_invalid_rgb", &[OUTPUT]),
    ("device_output_truncated", &[RGB, OUTPUT]),
    ("device_output_rgb_then_cmyk", &[RGB, IDENTITY]),
    ("device_output_cmyk_then_rgb", &[IDENTITY]),
    ("device_output_pdfa_rgb_then_wrong_s_cmyk", &[IDENTITY]),
    ("device_cmyk_image_rgb_output", &[CMYK]),
    ("device_cmyk_image_default", &[]),
    ("device_gray_image_no_output", &[GRAY]),
    ("device_gray_image_default", &[]),
    ("intent_annotation_appearance", &[INTENT]),
    ("intent_annotation_valid", &[]),
    ("intent_annotation_state", &[INTENT]),
    (
        "intent_annotation_down",
        &[INTENT, "PDFA1B-ANNOTATION-AP-ENTRIES-001"],
    ),
    ("intent_annotation_unreferenced", &[]),
    ("intent_pattern_content", &[INTENT]),
    ("intent_pattern_valid", &[]),
    ("intent_pattern_unused", &[]),
    ("intent_soft_mask_group", &["PDFA1B-EXTGSTATE-SMASK-001"]),
    ("intent_inline_image", &[INTENT]),
    ("intent_inline_valid", &[]),
    ("intent_inline_wrong_type", &[]),
    ("intent_alternate_image", &[INTENT, ALTERNATES]),
];

#[test]
fn unmodeled_color_paths_have_the_complete_expected_failure_delta() {
    common::assert_case_deltas(common::color_path_fixture, "icc_baseline", CASES);
}
