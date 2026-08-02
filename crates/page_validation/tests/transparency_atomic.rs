#[allow(dead_code)]
mod common;

const EXTGSTATE_SMASK: &str = "PDFA1B-EXTGSTATE-SMASK-001";
const XOBJECT_SMASK: &str = "PDFA1B-XOBJECT-SMASK-001";
const GROUP: &str = "PDFA1B-TRANSPARENCY-GROUP-001";
const BLEND_MODE: &str = "PDFA1B-EXTGSTATE-BLEND-MODE-001";
const STROKE_ALPHA: &str = "PDFA1B-EXTGSTATE-STROKE-ALPHA-001";
const FILL_ALPHA: &str = "PDFA1B-EXTGSTATE-FILL-ALPHA-001";

const CASES: &[(&str, &[&str])] = &[
    ("extgstate_smask_none", &[]),
    ("extgstate_smask_other", &[EXTGSTATE_SMASK]),
    ("extgstate_smask_dictionary", &[EXTGSTATE_SMASK]),
    ("extgstate_smask_null", &[]),
    ("extgstate_smask_indirect_null", &[EXTGSTATE_SMASK]),
    ("extgstate_bm_normal", &[]),
    ("extgstate_bm_compatible", &[]),
    ("extgstate_bm_multiply", &[BLEND_MODE]),
    ("extgstate_bm_null", &[]),
    ("extgstate_stroke_alpha_one", &[]),
    ("extgstate_stroke_alpha_zero", &[STROKE_ALPHA]),
    ("extgstate_fill_alpha_one", &[]),
    ("extgstate_fill_alpha_zero", &[FILL_ALPHA]),
    ("unused_extgstate_transparency", &[]),
    ("xobject_smask", &[XOBJECT_SMASK]),
    ("xobject_smask_null", &[]),
    ("xobject_smask_indirect_null", &[XOBJECT_SMASK]),
    ("unused_xobject_smask", &[]),
    ("page_transparency_group", &[GROUP]),
    ("page_nontransparency_group", &[]),
    ("form_transparency_group", &[GROUP]),
    ("unused_form_transparency_group", &[]),
];

#[test]
fn transparency_cases_have_the_complete_expected_failure_delta() {
    let baseline = common::failure_ids(&common::graphics_fixture("baseline"));
    for rule in [
        EXTGSTATE_SMASK,
        XOBJECT_SMASK,
        GROUP,
        BLEND_MODE,
        STROKE_ALPHA,
        FILL_ALPHA,
    ] {
        assert!(!baseline.contains(rule));
    }

    common::assert_case_deltas(common::graphics_fixture, "baseline", CASES);
}

#[test]
fn a_single_transparency_failure_attaches_its_owner() {
    let report = common::validate(&common::graphics_fixture("extgstate_bm_multiply"));
    let failure = common::assert_single_failure(&report, BLEND_MODE);
    assert!(failure.object_id.is_some());
}
