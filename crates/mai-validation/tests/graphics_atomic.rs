use std::collections::BTreeSet;

use mai_validation::{SafetyLimits, ValidationProfile, validate_bytes};

#[allow(dead_code)]
mod common;

const CASES: &[(&str, &[&str])] = &[
    ("extgstate_tr", &["PDFA1B-EXTGSTATE-TR-001"]),
    ("extgstate_tr2_default", &[]),
    ("extgstate_tr2_other", &["PDFA1B-EXTGSTATE-TR2-001"]),
    ("unused_extgstate_tr", &[]),
    ("unreferenced_extgstate_tr", &[]),
    ("ri_standard", &[]),
    ("ri_invalid", &["PDFA1B-RENDERING-INTENT-001"]),
    ("extgstate_ri_invalid", &["PDFA1B-RENDERING-INTENT-001"]),
    ("image_intent_valid", &[]),
    ("image_intent_invalid", &["PDFA1B-RENDERING-INTENT-001"]),
    ("undefined_operator", &["PDFA1B-CONTENT-OPERATOR-001"]),
    ("undefined_in_bx", &["PDFA1B-CONTENT-OPERATOR-001"]),
    ("known_operators", &[]),
    ("graphics_state_nesting_28", &[]),
    (
        "graphics_state_nesting_29",
        &["PDFA1B-GRAPHICS-STATE-NESTING-001"],
    ),
    ("undefined_form", &["PDFA1B-CONTENT-OPERATOR-001"]),
    ("unused_form_undefined", &[]),
];

#[test]
fn graphics_cases_have_the_complete_expected_failure_delta() {
    let baseline = failure_ids(&common::graphics_fixture("baseline"));
    for (case, expected_added) in CASES {
        let actual = failure_ids(&common::graphics_fixture(case));
        let (added, removed) = common::rule_delta(&baseline, &actual);
        let expected_added = expected_added
            .iter()
            .map(|rule_id| (*rule_id).to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(added, expected_added, "{case}: unexpected added failures");
        assert!(
            removed.is_empty(),
            "{case}: removed baseline failures {removed:?}"
        );
    }
}

#[test]
fn undefined_operator_failure_names_the_operator() {
    let report = validate_bytes(
        &common::graphics_fixture("undefined_operator"),
        ValidationProfile::PdfA1b,
        &SafetyLimits::default(),
    );
    let failure = report
        .failures
        .iter()
        .find(|failure| failure.rule_id == "PDFA1B-CONTENT-OPERATOR-001")
        .expect("undefined-operator failure");
    assert!(failure.message.contains("MaiUnknown"));
    assert_eq!(report.checks.total, 109);
    assert_eq!(report.checks.failed, 1);
    assert_eq!(report.checks.passed, 108);
}

fn failure_ids(bytes: &[u8]) -> BTreeSet<String> {
    validate_bytes(bytes, ValidationProfile::PdfA1b, &SafetyLimits::default())
        .failures
        .into_iter()
        .map(|failure| failure.rule_id.to_owned())
        .collect()
}
