pub mod common;

#[test]
fn a_single_indirect_form_failure_attaches_its_owner() {
    for (case, rule_id) in [
        (
            "need_appearances_true",
            "PDFA1B-ACROFORM-NEED-APPEARANCES-001",
        ),
        ("widget_missing_ap", "PDFA1B-WIDGET-APPEARANCE-001"),
    ] {
        let report = common::validate(&common::form_fixture(case));
        let failure = common::assert_single_failure(&report, rule_id);
        assert!(failure.object_id.is_some(), "{case}: missing object ID");
    }
}
