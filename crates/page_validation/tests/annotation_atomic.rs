pub mod common;

const FLAGS: &str = "PDFA1B-ANNOTATION-FLAGS-001";

#[test]
fn indirect_annotation_failure_is_reported() {
    let report = common::validate(&common::annotation_fixture("flags_missing"));
    common::assert_single_failure(&report, FLAGS);
}
