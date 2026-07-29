use std::collections::BTreeSet;

use mai_validation::{SafetyLimits, ValidationProfile, validate_bytes};

#[allow(dead_code)]
mod common;

#[test]
fn form_cases_have_the_complete_expected_failure_delta() {
    let cases = [
        ("baseline", &[][..]),
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
    let baseline = validate_bytes(
        &common::form_fixture("baseline"),
        ValidationProfile::PdfA1b,
        &SafetyLimits::default(),
    );
    let baseline_ids = baseline
        .failures
        .iter()
        .map(|failure| failure.rule_id)
        .collect::<BTreeSet<_>>();
    for (case, expected) in cases {
        let report = validate_bytes(
            &common::form_fixture(case),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        let case_ids = report
            .failures
            .iter()
            .map(|failure| failure.rule_id)
            .collect::<BTreeSet<_>>();
        let (added, removed) = common::rule_delta(&baseline_ids, &case_ids);
        assert_eq!(
            added,
            expected.iter().copied().collect(),
            "{case}: unexpected failure delta: {:#?}",
            report.failures
        );
        assert!(removed.is_empty(), "{case}: removed baseline failures");
    }
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
        let report = validate_bytes(
            &common::form_fixture(case),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        let failure = report
            .failures
            .iter()
            .find(|failure| failure.rule_id == rule_id)
            .expect("targeted form failure");
        assert!(failure.object_id.is_some(), "{case}: missing object ID");
        assert_eq!(report.checks.total, 99);
        assert_eq!(report.checks.failed, 1);
        assert_eq!(report.checks.passed, 98);
    }
}
