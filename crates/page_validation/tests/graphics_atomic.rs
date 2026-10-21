pub mod common;

#[test]
fn undefined_operator_failure_names_the_operator() {
    let report = common::validate(&common::graphics_fixture("undefined_operator"));
    let failure = common::assert_single_failure(&report, "ISO 19005-1:2005:6.2.10:1");
    assert!(failure.message.contains("MaiUnknown"));
}
