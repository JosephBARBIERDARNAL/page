pub mod common;

#[test]
fn undefined_operator_failure_names_the_operator() {
    let report = common::validate(&common::graphics_fixture("undefined_operator"));
    let failure = common::assert_single_failure(&report, "PDFA1B-CONTENT-OPERATOR-001");
    assert!(failure.message.contains("MaiUnknown"));
}
