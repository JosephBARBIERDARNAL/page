pub mod common;

const FLAGS: &str = "PDFA1B-ANNOTATION-FLAGS-001";

#[test]
fn indirect_annotation_failure_attaches_the_annotation_object() {
    let report = common::validate(&common::annotation_fixture("flags_missing"));
    let failure = common::assert_single_failure(&report, FLAGS);
    assert!(failure.object_id.is_some());
}
