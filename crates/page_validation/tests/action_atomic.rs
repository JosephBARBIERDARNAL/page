mod common;

use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

const ACTION_TYPE: &str = "PDFA1B-ACTION-TYPE-001";
const NAMED_ACTION: &str = "PDFA1B-NAMED-ACTION-001";
const WIDGET_ACTION: &str = "PDFA1B-WIDGET-ACTION-001";
const WIDGET_AA: &str = "PDFA1B-WIDGET-ADDITIONAL-ACTIONS-001";
const FIELD_AA: &str = "PDFA1B-FIELD-ADDITIONAL-ACTIONS-001";
const CATALOG_AA: &str = "PDFA1B-CATALOG-ADDITIONAL-ACTIONS-001";
const FILE_SPEC: &str = "PDFA1B-FILE-SPEC-EMBEDDED-FILE-001";

const CASES: &[(&str, &[&str])] = &[
    ("allowed_goto", &[]),
    ("allowed_gotor", &[]),
    ("allowed_thread", &[]),
    ("allowed_uri", &[]),
    ("allowed_named", &[]),
    ("allowed_submit_form", &[]),
    ("gotor_action_with_ef_file_spec", &[FILE_SPEC]),
    ("submit_form_action_with_ef_file_spec", &[FILE_SPEC]),
    ("gotor_action_without_ef_file_spec", &[]),
    ("open_javascript", &[ACTION_TYPE]),
    ("open_javascript_indirect", &[ACTION_TYPE]),
    ("open_missing_subtype", &[ACTION_TYPE]),
    ("open_wrong_subtype_type", &[ACTION_TYPE]),
    ("open_indirect_subtype", &[ACTION_TYPE]),
    ("open_destination_array", &[]),
    ("unreferenced_javascript", &[]),
    ("page_additional_action", &[ACTION_TYPE]),
    ("page_unknown_additional_action", &[]),
    ("annotation_action", &[ACTION_TYPE]),
    ("annotation_additional_action", &[ACTION_TYPE]),
    ("annotation_unknown_additional_action", &[]),
    ("outline_action", &[ACTION_TYPE]),
    ("outline_stream_action", &[ACTION_TYPE]),
    ("outline_stream_node_action", &[ACTION_TYPE]),
    ("next_action", &[ACTION_TYPE]),
    ("next_action_array", &[ACTION_TYPE]),
    ("next_stream_action_ignored", &[]),
    ("next_action_cycle", &[]),
    ("named_next_page", &[]),
    ("named_prev_page", &[]),
    ("named_first_page", &[]),
    ("named_last_page", &[]),
    ("named_forbidden", &[NAMED_ACTION]),
    ("named_missing", &[NAMED_ACTION]),
    ("named_wrong_type", &[NAMED_ACTION]),
    ("named_indirect_forbidden", &[NAMED_ACTION]),
    ("non_named_with_forbidden_n", &[]),
    ("widget_action", &[WIDGET_ACTION]),
    ("widget_action_wrong_type", &[WIDGET_ACTION]),
    ("widget_a_null", &[]),
    ("widget_indirect_subtype_action", &[WIDGET_ACTION]),
    ("stream_widget_action_ignored", &[]),
    ("widget_additional_actions", &[WIDGET_AA]),
    ("widget_aa_null", &[]),
    ("widget_additional_javascript", &[ACTION_TYPE, WIDGET_AA]),
    ("text_additional_actions", &[]),
    ("field_additional_actions", &[FIELD_AA]),
    ("direct_field_additional_actions", &[FIELD_AA]),
    ("field_aa_null", &[]),
    ("field_additional_javascript", &[ACTION_TYPE, FIELD_AA]),
    ("stream_acroform_field_additional_actions", &[FIELD_AA]),
    ("stream_field_additional_actions", &[FIELD_AA]),
    ("stream_field_aa_javascript", &[ACTION_TYPE, FIELD_AA]),
    ("child_field_additional_actions", &[FIELD_AA]),
    ("child_without_t", &[]),
    ("stream_child_field_additional_actions", &[FIELD_AA]),
    ("unnamed_child_reused_as_top_field", &[FIELD_AA]),
    ("field_cycle", &[FIELD_AA]),
    ("top_field_without_t", &[FIELD_AA]),
    ("unreferenced_field_additional_actions", &[]),
    (
        "combined_widget_field_actions",
        &[WIDGET_ACTION, WIDGET_AA, FIELD_AA],
    ),
    ("catalog_additional_actions", &[CATALOG_AA]),
    ("catalog_aa_null", &[]),
    ("catalog_additional_javascript", &[ACTION_TYPE, CATALOG_AA]),
    ("catalog_unknown_additional_action", &[CATALOG_AA]),
    ("catalog_additional_actions_wrong_type", &[CATALOG_AA]),
];

#[test]
fn action_cases_have_the_complete_expected_failure_delta() {
    let baseline = common::failure_ids(&common::action_fixture("baseline"));
    for rule in [
        ACTION_TYPE,
        NAMED_ACTION,
        WIDGET_ACTION,
        WIDGET_AA,
        FIELD_AA,
        CATALOG_AA,
        FILE_SPEC,
    ] {
        assert!(!baseline.contains(rule));
    }
    common::assert_case_deltas(common::action_fixture, "baseline", CASES);
}

#[test]
fn indirect_action_failure_attaches_the_action_object() {
    let report = common::validate(&common::action_fixture("open_javascript_indirect"));
    let failure = common::assert_single_failure(&report, ACTION_TYPE);
    assert!(failure.object_id.is_some());
}

#[test]
fn cyclic_field_graph_terminates_under_the_configured_reference_limit() {
    let limits = SafetyLimits {
        max_reference_depth: 4,
        ..SafetyLimits::default()
    };
    let report = validate_bytes_with_profile(
        &common::action_fixture("field_cycle"),
        ValidationProfile::PdfA1b,
        &limits,
    );

    assert_eq!(report.exit_code(), 2, "{report:#?}");
    assert_eq!(
        report
            .failures
            .iter()
            .filter(|failure| failure.rule_id == FIELD_AA)
            .count(),
        1,
        "{report:#?}"
    );
}
