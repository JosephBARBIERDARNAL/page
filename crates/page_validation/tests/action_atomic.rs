pub mod common;

use page_validation::{SafetyLimits, ValidationOptions, ValidationProfile, validate_pdf_bytes};

const ACTION_TYPE: &str = "ISO 19005-1:2005:6.6.1:1";
const FIELD_AA: &str = "ISO 19005-1:2005:6.6.2:2";

#[test]
fn indirect_action_failure_is_reported() {
    let report = common::validate(&common::action_fixture("open_javascript_indirect"));
    common::assert_single_failure(&report, ACTION_TYPE);
}

#[test]
fn cyclic_field_graph_terminates_under_the_configured_reference_limit() {
    let limits = SafetyLimits::default().with_max_reference_depth(4);
    let report = validate_pdf_bytes(
        &common::action_fixture("field_cycle"),
        &ValidationOptions::default()
            .profile(ValidationProfile::PdfA1b)
            .limits(limits),
    )
    .expect("explicit profile validation");

    assert!(!report.is_compliant, "{report:#?}");
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
