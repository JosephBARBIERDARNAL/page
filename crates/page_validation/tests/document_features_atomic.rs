use page_validation::ValidationProfile;

pub mod common;

const EMBEDDED_FILES: &str = "ISO 19005-1:2005:6.1.11:2";
const OPTIONAL_CONTENT: &str = "ISO 19005-1:2005:6.1.13:1";
const FILE_SPEC: &str = "ISO 19005-1:2005:6.1.11:1";
const STREAM_EXTERNAL: &str = "ISO 19005-1:2005:6.1.7:3";

#[test]
fn document_feature_failures_are_reported_and_counted() {
    for (case, rule_id) in [
        ("names_embedded_files_dictionary", EMBEDDED_FILES),
        ("ocproperties_dictionary", OPTIONAL_CONTENT),
        ("file_spec_indirect_with_ef", FILE_SPEC),
        ("stream_f", STREAM_EXTERNAL),
    ] {
        let report = common::validate(&common::document_feature_fixture(case));
        report
            .failures
            .iter()
            .find(|failure| failure.rule_id == rule_id)
            .expect("targeted document-feature failure");
        let total = ValidationProfile::PdfA1b.implemented_check_count();
        assert_eq!(report.rules.total, total);
        let expected_failed = if rule_id == FILE_SPEC { 2 } else { 1 };
        assert_eq!(report.rules.failed, expected_failed);
        assert_eq!(report.rules.passed, total - expected_failed);
    }
}
