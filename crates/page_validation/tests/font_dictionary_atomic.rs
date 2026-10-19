pub mod common;

const BASE_FONT: &str = "PDFA1B-FONT-BASEFONT-001";
#[test]
fn a_single_invalid_font_failure_is_reported() {
    let report = common::validate(&common::font_fixture("font_basefont_missing"));
    common::assert_single_failure(&report, BASE_FONT);
}
