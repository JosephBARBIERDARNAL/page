pub mod common;

const BLEND_MODE: &str = "PDFA1B-EXTGSTATE-BLEND-MODE-001";

#[test]
fn a_single_transparency_failure_attaches_its_owner() {
    let report = common::validate(&common::graphics_fixture("extgstate_bm_multiply"));
    let failure = common::assert_single_failure(&report, BLEND_MODE);
    assert!(failure.object_id.is_some());
}
