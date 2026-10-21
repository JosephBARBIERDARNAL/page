pub mod common;

const BASE_FONT: &str = "ISO 19005-1:2005:6.3.2:3";
#[test]
fn a_single_invalid_font_failure_is_reported() {
    let report = common::validate(&common::font_fixture("font_basefont_missing"));
    common::assert_single_failure(&report, BASE_FONT);
}
