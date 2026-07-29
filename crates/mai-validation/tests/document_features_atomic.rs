#[allow(dead_code)]
mod common;

const EMBEDDED_FILES: &str = "PDFA1B-NAMES-EMBEDDED-FILES-001";
const OPTIONAL_CONTENT: &str = "PDFA1B-OPTIONAL-CONTENT-001";
const FILE_SPEC: &str = "PDFA1B-FILE-SPEC-EMBEDDED-FILE-001";
const STREAM_EXTERNAL: &str = "PDFA1B-STREAM-EXTERNAL-DATA-001";
const STREAM_LZW: &str = "PDFA1B-STREAM-LZW-001";

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
        ("file_spec_without_ef", &[EMBEDDED_FILES]),
        ("file_spec_with_ef", &[EMBEDDED_FILES, FILE_SPEC]),
        ("file_spec_indirect_with_ef", &[EMBEDDED_FILES, FILE_SPEC]),
        ("file_spec_stream_with_ef", &[EMBEDDED_FILES, FILE_SPEC]),
        ("file_spec_scalar", &[EMBEDDED_FILES]),
        ("embedded_files_kids_with_ef", &[EMBEDDED_FILES, FILE_SPEC]),
        ("stream_f", &[STREAM_EXTERNAL]),
        ("stream_ffilter", &[STREAM_EXTERNAL]),
        ("stream_fdecodeparms", &[STREAM_EXTERNAL]),
        ("stream_external_null", &[]),
        ("stream_lzwdecode", &[STREAM_LZW]),
        ("stream_lzwdecode_array", &[STREAM_LZW]),
        ("stream_lzwdecode_indirect", &[STREAM_LZW]),
        ("stream_lzw_short_name", &[]),
        ("ocproperties_dictionary", &[OPTIONAL_CONTENT]),
        ("ocproperties_wrong_type", &[OPTIONAL_CONTENT]),
        ("ocproperties_null", &[]),
        ("ocproperties_indirect_null", &[OPTIONAL_CONTENT]),
        ("ocproperties_stream", &[OPTIONAL_CONTENT]),
        ("unreferenced_catalog_ocproperties", &[]),
    ];
    let baseline = common::failure_ids(&common::document_feature_fixture("baseline"));
    for rule in [EMBEDDED_FILES, OPTIONAL_CONTENT] {
        assert!(!baseline.contains(rule));
    }
    for (case, expected) in cases {
        let actual = common::failure_ids(&common::document_feature_fixture(case));
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
        assert_eq!(report.checks.total, 127);
        let expected_failed = if rule_id == FILE_SPEC { 2 } else { 1 };
        assert_eq!(report.checks.failed, expected_failed);
        assert_eq!(report.checks.passed, 127 - expected_failed);
    }
}
