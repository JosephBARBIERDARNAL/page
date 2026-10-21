pub mod common;

const FLAGS: &str = "ISO 19005-1:2005:6.5.3:2";

#[test]
fn indirect_annotation_failure_is_reported() {
    let report = common::validate(&common::annotation_fixture("flags_missing"));
    common::assert_single_failure(&report, FLAGS);
}
