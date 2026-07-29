use std::collections::BTreeSet;

use mai_validation::{SafetyLimits, ValidationProfile, validate_bytes};

#[allow(dead_code)]
mod common;

const EMBEDDED_FILES: &str = "PDFA1B-NAMES-EMBEDDED-FILES-001";
const OPTIONAL_CONTENT: &str = "PDFA1B-OPTIONAL-CONTENT-001";

#[test]
fn document_feature_cases_have_the_complete_expected_failure_delta() {
    let cases = [
        ("baseline", &[][..]),
        ("names_empty", &[]),
        ("names_embedded_files_dictionary", &[EMBEDDED_FILES]),
        ("names_embedded_files_wrong_type", &[EMBEDDED_FILES]),
        ("names_embedded_files_null", &[]),
        ("names_embedded_files_indirect_null", &[EMBEDDED_FILES]),
        ("names_stream_embedded_files", &[]),
        ("names_wrong_type", &[]),
        ("names_indirect_dictionary", &[EMBEDDED_FILES]),
        ("unreferenced_names_embedded_files", &[]),
        ("ocproperties_dictionary", &[OPTIONAL_CONTENT]),
        ("ocproperties_wrong_type", &[OPTIONAL_CONTENT]),
        ("ocproperties_null", &[]),
        ("ocproperties_indirect_null", &[OPTIONAL_CONTENT]),
        ("ocproperties_stream", &[OPTIONAL_CONTENT]),
        ("unreferenced_catalog_ocproperties", &[]),
    ];
    let baseline = failure_ids(&common::document_feature_fixture("baseline"));
    for rule in [EMBEDDED_FILES, OPTIONAL_CONTENT] {
        assert!(!baseline.contains(rule));
    }
    for (case, expected) in cases {
        let actual = failure_ids(&common::document_feature_fixture(case));
        let (added, removed) = common::rule_delta(&baseline, &actual);
        assert_eq!(
            added,
            expected.iter().map(|rule| (*rule).to_owned()).collect(),
            "{case}: unexpected added failures"
        );
        assert!(
            removed.is_empty(),
            "{case}: removed baseline failures {removed:?}"
        );
    }
}

#[test]
fn document_feature_failures_attach_the_catalog_object() {
    for (case, rule_id) in [
        ("names_embedded_files_dictionary", EMBEDDED_FILES),
        ("ocproperties_dictionary", OPTIONAL_CONTENT),
    ] {
        let report = validate(&common::document_feature_fixture(case));
        let failure = report
            .failures
            .iter()
            .find(|failure| failure.rule_id == rule_id)
            .expect("targeted document-feature failure");
        assert!(
            failure.object_id.is_some(),
            "{case}: missing catalog object ID"
        );
        assert_eq!(report.checks.total, 98);
        assert_eq!(report.checks.failed, 1);
        assert_eq!(report.checks.passed, 97);
    }
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
