use page_validation::ValidationProfile;

pub mod common;

const EMBEDDED_FILES: &str = "PDFA1B-NAMES-EMBEDDED-FILES-001";
const OPTIONAL_CONTENT: &str = "PDFA1B-OPTIONAL-CONTENT-001";
const FILE_SPEC: &str = "PDFA1B-FILE-SPEC-EMBEDDED-FILE-001";
const STREAM_EXTERNAL: &str = "PDFA1B-STREAM-EXTERNAL-DATA-001";

#[test]
fn document_feature_failures_attach_the_catalog_object() {
    for (case, rule_id) in [
        ("names_embedded_files_dictionary", EMBEDDED_FILES),
        ("ocproperties_dictionary", OPTIONAL_CONTENT),
        ("file_spec_indirect_with_ef", FILE_SPEC),
        ("stream_f", STREAM_EXTERNAL),
    ] {
        let report = common::validate(&common::document_feature_fixture(case));
        let failure = report
            .failures
            .iter()
            .find(|failure| failure.rule_id == rule_id)
            .expect("targeted document-feature failure");
        assert!(
            failure.object_id.is_some(),
            "{case}: missing catalog object ID"
        );
        let total = ValidationProfile::PdfA1b.implemented_check_count();
        assert_eq!(report.checks.total, total);
        let expected_failed = if rule_id == FILE_SPEC { 2 } else { 1 };
        assert_eq!(report.checks.failed, expected_failed);
        assert_eq!(report.checks.passed, total - expected_failed);
    }
}
