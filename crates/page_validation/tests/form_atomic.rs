pub mod common;

#[test]
fn a_single_indirect_form_failure_is_reported() {
    for (case, rule_id) in [
        ("need_appearances_true", "ISO 19005-1:2005:6.9:1"),
        ("widget_missing_ap", "ISO 19005-1:2005:6.9:2"),
    ] {
        let report = common::validate(&common::form_fixture(case));
        common::assert_single_failure(&report, rule_id);
    }
}
