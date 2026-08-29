pub mod common;

use page_validation::{SafetyLimits, ValidationProfile, validate_pdf_bytes};

const ACTION_TYPE: &str = "PDFA1B-ACTION-TYPE-001";
const FIELD_AA: &str = "PDFA1B-FIELD-ADDITIONAL-ACTIONS-001";

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
    let report = validate_pdf_bytes(
        &common::action_fixture("field_cycle"),
        Some(ValidationProfile::PdfA1b),
        &limits,
    )
    .expect("explicit profile validation");

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
