pub mod common;

const BLEND_MODE: &str = "ISO 19005-1:2005:6.4:4";

#[test]
fn a_single_transparency_failure_is_reported() {
    let report = common::validate(&common::graphics_fixture("extgstate_bm_multiply"));
    common::assert_single_failure(&report, BLEND_MODE);
}
