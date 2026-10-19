pub mod common;

#[test]
fn a_single_composite_font_failure_is_reported() {
    let report = common::validate(&common::font_fixture("composite_cidmap_missing"));
    common::assert_single_failure(&report, "PDFA1B-CIDTOGIDMAP-001");
}
