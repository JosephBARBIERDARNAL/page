#[allow(dead_code)]
mod common;

const CASES: &[(&str, &[&str])] = &[
    ("extgstate_tr", &["PDFA1B-EXTGSTATE-TR-001"]),
    ("direct_extgstate_tr", &["PDFA1B-EXTGSTATE-TR-001"]),
    ("extgstate_tr2_default", &[]),
    ("extgstate_tr2_other", &["PDFA1B-EXTGSTATE-TR2-001"]),
    ("unused_extgstate_tr", &[]),
    ("unreferenced_extgstate_tr", &[]),
    ("ri_standard", &[]),
    ("ri_invalid", &["PDFA1B-RENDERING-INTENT-001"]),
    ("extgstate_ri_invalid", &["PDFA1B-RENDERING-INTENT-001"]),
    ("image_intent_valid", &[]),
    ("image_intent_invalid", &["PDFA1B-RENDERING-INTENT-001"]),
    ("undefined_operator", &["PDFA1B-CONTENT-OPERATOR-001"]),
    ("undefined_in_bx", &["PDFA1B-CONTENT-OPERATOR-001"]),
    ("inline_image_lzw", &["PDFA1B-INLINE-IMAGE-LZW-001"]),
    ("inline_image_lzw_array", &["PDFA1B-INLINE-IMAGE-LZW-001"]),
    ("inline_image_ascii_hex", &[]),
    ("known_operators", &[]),
    ("graphics_state_nesting_28", &[]),
    (
        "graphics_state_nesting_29",
        &["PDFA1B-GRAPHICS-STATE-NESTING-001"],
    ),
    ("undefined_form", &["PDFA1B-CONTENT-OPERATOR-001"]),
    ("unused_form_undefined", &[]),
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
