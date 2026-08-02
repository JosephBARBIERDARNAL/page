#[allow(dead_code)]
mod common;

const CASES: &[(&str, &[&str])] = &[
    ("extgstate_tr", &["PDFA1B-EXTGSTATE-TR-001"]),
    ("direct_extgstate_tr", &["PDFA1B-EXTGSTATE-TR-001"]),
    (
        "indirect_extgstate_resource_dictionary",
        &["PDFA1B-EXTGSTATE-TR-001"],
    ),
    ("inherited_extgstate_tr", &["PDFA1B-EXTGSTATE-TR-001"]),
    ("extgstate_tr_null", &[]),
    ("extgstate_tr_indirect_null", &["PDFA1B-EXTGSTATE-TR-001"]),
    ("extgstate_tr2_default", &[]),
    ("extgstate_tr2_other", &["PDFA1B-EXTGSTATE-TR2-001"]),
    ("extgstate_tr2_null", &[]),
    ("unused_extgstate_tr", &[]),
    ("unreferenced_extgstate_tr", &[]),
    ("ri_standard", &[]),
    ("ri_invalid", &["PDFA1B-RENDERING-INTENT-001"]),
    ("extgstate_ri_invalid", &["PDFA1B-RENDERING-INTENT-001"]),
    ("image_intent_valid", &[]),
    ("image_intent_invalid", &["PDFA1B-RENDERING-INTENT-001"]),
    ("explicit_mask_image_intent_invalid", &[]),
    (
        "soft_mask_image_intent_invalid",
        &["PDFA1B-RENDERING-INTENT-001", "PDFA1B-XOBJECT-SMASK-001"],
    ),
    ("undefined_operator", &["PDFA1B-CONTENT-OPERATOR-001"]),
    ("undefined_in_bx", &["PDFA1B-CONTENT-OPERATOR-001"]),
    ("undefined_before_malformed_array", &[]),
    ("malformed_string_before_undefined", &[]),
    ("unmatched_graphics_restore", &[]),
    ("gs_wrong_operand", &[]),
    ("gs_extra_operand", &[]),
    ("inline_image_lzw", &["PDFA1B-INLINE-IMAGE-LZW-001"]),
    ("inline_image_lzw_array", &["PDFA1B-INLINE-IMAGE-LZW-001"]),
    ("inline_image_lzw_escaped", &["PDFA1B-INLINE-IMAGE-LZW-001"]),
    (
        "inline_image_unterminated_lzw",
        &["PDFA1B-INLINE-IMAGE-LZW-001"],
    ),
    ("inline_image_ascii_hex", &[]),
    ("inline_image_false_ei", &[]),
    ("inline_image_tokens_in_string", &[]),
    ("known_operators", &[]),
    ("graphics_state_nesting_28", &[]),
    (
        "graphics_state_nesting_29",
        &["PDFA1B-GRAPHICS-STATE-NESTING-001"],
    ),
    ("undefined_form", &["PDFA1B-CONTENT-OPERATOR-001"]),
    ("unused_form_undefined", &[]),
    ("undefined_appearance", &["PDFA1B-CONTENT-OPERATOR-001"]),
    ("unused_appearance_undefined", &[]),
    ("undefined_pattern", &["PDFA1B-CONTENT-OPERATOR-001"]),
    ("unused_pattern_undefined", &[]),
    ("malformed_pattern_leading_name", &[]),
    ("malformed_pattern_trailing_number", &[]),
    ("malformed_pattern_colorspace", &[]),
    ("shading_pattern_extgstate_tr", &[]),
    ("direct_shading_pattern_extgstate_tr", &[]),
    ("unused_shading_pattern_extgstate_tr", &[]),
    ("undefined_type3", &["PDFA1B-CONTENT-OPERATOR-001"]),
    ("unused_type3_undefined", &[]),
    ("undefined_soft_mask_group", &["PDFA1B-EXTGSTATE-SMASK-001"]),
    ("unused_soft_mask_group_undefined", &[]),
];

#[test]
fn graphics_cases_have_the_complete_expected_failure_delta() {
    common::assert_case_deltas(common::graphics_fixture, "baseline", CASES);
}

#[test]
fn undefined_operator_failure_names_the_operator() {
    let report = common::validate(&common::graphics_fixture("undefined_operator"));
    let failure = common::assert_single_failure(&report, "PDFA1B-CONTENT-OPERATOR-001");
    assert!(failure.message.contains("MaiUnknown"));
}
