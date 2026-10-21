pub mod common;

#[test]
fn a_single_composite_font_failure_is_reported() {
    let report = common::validate(&common::font_fixture("composite_cidmap_missing"));
    common::assert_single_failure(&report, "ISO 19005-1:2005:6.3.3.2:1");
}
