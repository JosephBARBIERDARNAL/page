use std::collections::BTreeSet;

use mai_validation::{SafetyLimits, ValidationProfile, validate_bytes};

#[allow(dead_code)]
mod common;

const ACTION_TYPE: &str = "PDFA1B-ACTION-TYPE-001";
const NAMED_ACTION: &str = "PDFA1B-NAMED-ACTION-001";
const WIDGET_ACTION: &str = "PDFA1B-WIDGET-ACTION-001";
const WIDGET_AA: &str = "PDFA1B-WIDGET-ADDITIONAL-ACTIONS-001";
const FIELD_AA: &str = "PDFA1B-FIELD-ADDITIONAL-ACTIONS-001";
const CATALOG_AA: &str = "PDFA1B-CATALOG-ADDITIONAL-ACTIONS-001";

const CASES: &[(&str, &[&str])] = &[
    ("allowed_goto", &[]),
    ("allowed_gotor", &[]),
    ("allowed_thread", &[]),
    ("allowed_uri", &[]),
    ("allowed_named", &[]),
    ("allowed_submit_form", &[]),
    ("open_javascript", &[ACTION_TYPE]),
    ("open_javascript_indirect", &[ACTION_TYPE]),
    ("open_missing_subtype", &[ACTION_TYPE]),
    ("open_wrong_subtype_type", &[ACTION_TYPE]),
    ("open_destination_array", &[]),
    ("unreferenced_javascript", &[]),
    ("page_additional_action", &[ACTION_TYPE]),
    ("page_unknown_additional_action", &[]),
    ("annotation_action", &[ACTION_TYPE]),
    ("annotation_additional_action", &[ACTION_TYPE]),
    ("annotation_unknown_additional_action", &[]),
    ("outline_action", &[ACTION_TYPE]),
    ("next_action", &[ACTION_TYPE]),
    ("next_action_array", &[ACTION_TYPE]),
    ("named_next_page", &[]),
    ("named_prev_page", &[]),
    ("named_first_page", &[]),
    ("named_last_page", &[]),
    ("named_forbidden", &[NAMED_ACTION]),
    ("named_missing", &[NAMED_ACTION]),
    ("named_wrong_type", &[NAMED_ACTION]),
    ("non_named_with_forbidden_n", &[]),
    ("widget_action", &[WIDGET_ACTION]),
    ("widget_action_wrong_type", &[WIDGET_ACTION]),
    ("widget_additional_actions", &[WIDGET_AA]),
    ("widget_additional_javascript", &[ACTION_TYPE, WIDGET_AA]),
    ("text_additional_actions", &[]),
    ("field_additional_actions", &[FIELD_AA]),
    ("field_additional_javascript", &[ACTION_TYPE, FIELD_AA]),
    ("child_field_additional_actions", &[FIELD_AA]),
    ("child_without_t", &[]),
    ("top_field_without_t", &[FIELD_AA]),
    ("unreferenced_field_additional_actions", &[]),
    (
        "combined_widget_field_actions",
        &[WIDGET_ACTION, WIDGET_AA, FIELD_AA],
    ),
    ("catalog_additional_actions", &[CATALOG_AA]),
    ("catalog_additional_javascript", &[ACTION_TYPE, CATALOG_AA]),
    ("catalog_unknown_additional_action", &[CATALOG_AA]),
    ("catalog_additional_actions_wrong_type", &[CATALOG_AA]),
];

#[test]
fn action_cases_have_the_complete_expected_failure_delta() {
    let baseline = failure_ids(&common::action_fixture("baseline"));
    for rule in [
        ACTION_TYPE,
        NAMED_ACTION,
        WIDGET_ACTION,
        WIDGET_AA,
        FIELD_AA,
        CATALOG_AA,
    ] {
        assert!(!baseline.contains(rule));
    }
    for (case, expected) in CASES {
        let actual = failure_ids(&common::action_fixture(case));
        let (added, removed) = common::rule_delta(&baseline, &actual);
        assert_eq!(
            added,
            expected
                .iter()
                .map(|rule| (*rule).to_owned())
                .collect::<BTreeSet<_>>(),
            "{case}: unexpected added failures"
        );
        assert!(
            removed.is_empty(),
            "{case}: removed baseline failures {removed:?}"
        );
    }
}

#[test]
fn indirect_action_failure_attaches_the_action_object() {
    let report = validate(&common::action_fixture("open_javascript_indirect"));
    let failure = report
        .failures
        .iter()
        .find(|failure| failure.rule_id == ACTION_TYPE)
        .expect("action type failure");
    assert!(failure.object_id.is_some());
    assert_eq!(report.checks.total, 108);
    assert_eq!(report.checks.failed, 1);
    assert_eq!(report.checks.passed, 107);
}

fn validate(bytes: &[u8]) -> mai_validation::ValidationReport {
    validate_bytes(bytes, ValidationProfile::PdfA1b, &SafetyLimits::default())
}

fn failure_ids(bytes: &[u8]) -> BTreeSet<String> {
    validate(bytes)
        .failures
        .into_iter()
        .map(|failure| failure.rule_id.to_owned())
        .collect()
}
