pub mod common;

#[test]
fn a_single_composite_failure_attaches_the_type0_font() {
    let report = common::validate(&common::font_fixture("composite_cidmap_missing"));
    let failure = common::assert_single_failure(&report, "PDFA1B-CIDTOGIDMAP-001");
    assert!(failure.object_id.is_some());
}
