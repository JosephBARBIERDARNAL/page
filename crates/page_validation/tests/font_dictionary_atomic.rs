pub mod common;

const BASE_FONT: &str = "PDFA1B-FONT-BASEFONT-001";
#[test]
fn a_single_invalid_font_attaches_the_font_object() {
    let report = common::validate(&common::font_fixture("font_basefont_missing"));
    let failure = common::assert_single_failure(&report, BASE_FONT);
    assert!(failure.object_id.is_some());
}
