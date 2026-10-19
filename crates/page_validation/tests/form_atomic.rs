pub mod common;

#[test]
fn a_single_indirect_form_failure_is_reported() {
    for (case, rule_id) in [
        (
            "need_appearances_true",
            "PDFA1B-ACROFORM-NEED-APPEARANCES-001",
        ),
        ("widget_missing_ap", "PDFA1B-WIDGET-APPEARANCE-001"),
    ] {
        let report = common::validate(&common::form_fixture(case));
        common::assert_single_failure(&report, rule_id);
    }
}
