mod common;

const CASES: &[(&str, &[&str])] = &[
    ("baseline", &[]),
    ("no_acroform", &[]),
    ("need_appearances_absent", &[]),
    ("need_appearances_false_indirect", &[]),
    (
        "need_appearances_true",
        &["PDFA1B-ACROFORM-NEED-APPEARANCES-001"],
    ),
    (
        "need_appearances_true_indirect",
        &["PDFA1B-ACROFORM-NEED-APPEARANCES-001"],
    ),
    (
        "need_appearances_wrong_type",
        &["PDFA1B-ACROFORM-NEED-APPEARANCES-001"],
    ),
    ("need_appearances_null", &[]),
    ("acroform_wrong_type", &[]),
    (
        "acroform_stream_true",
        &["PDFA1B-ACROFORM-NEED-APPEARANCES-001"],
    ),
    ("widget_missing_ap", &["PDFA1B-WIDGET-APPEARANCE-001"]),
    (
        "widget_indirect_subtype_missing_ap",
        &["PDFA1B-WIDGET-APPEARANCE-001"],
    ),
    ("stream_widget_missing_ap", &[]),
    ("widget_empty_ap", &["PDFA1B-ANNOTATION-AP-ENTRIES-001"]),
    ("widget_wrong_type_ap", &["PDFA1B-WIDGET-APPEARANCE-001"]),
    ("widget_stream_ap", &["PDFA1B-WIDGET-APPEARANCE-001"]),
    ("widget_indirect_ap", &[]),
    ("non_widget_missing_ap", &[]),
    ("field_only_widget_missing_ap", &[]),
    (
        "direct_widget_missing_ap",
        &["PDFA1B-WIDGET-APPEARANCE-001"],
    ),
    ("widget_parent_ap_only", &["PDFA1B-WIDGET-APPEARANCE-001"]),
    ("unreferenced_widget_missing_ap", &[]),
];

#[test]
fn form_cases_have_the_complete_expected_failure_delta() {
    common::assert_case_deltas(common::form_fixture, "baseline", CASES);
}

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
