use std::{env, fs};

use page_validation::differential::{
    ComparisonClassification, DifferentialRunner, ReferenceConfig, ReferenceProfile,
};
use page_validation::{SafetyLimits, ValidationProfile, validate_pdf_bytes};

pub mod common;

const EMBEDDED_FILES: &str = "PDFA1B-NAMES-EMBEDDED-FILES-001";
const OPTIONAL_CONTENT: &str = "PDFA1B-OPTIONAL-CONTENT-001";
const FILE_SPEC: &str = "PDFA1B-FILE-SPEC-EMBEDDED-FILE-001";
const STREAM_EXTERNAL: &str = "PDFA1B-STREAM-EXTERNAL-DATA-001";
const STREAM_LZW: &str = "PDFA1B-STREAM-LZW-001";

const CASES: &[(&str, &[&str])] = &[
    ("baseline", &[]),
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
    ("file_spec_direct_null_ef", &[EMBEDDED_FILES]),
    ("file_spec_indirect_null_ef", &[EMBEDDED_FILES, FILE_SPEC]),
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

#[test]
fn document_feature_cases_have_the_complete_expected_failure_delta() {
    let baseline = common::failure_ids(&common::document_feature_fixture("baseline"));
    for rule in [EMBEDDED_FILES, OPTIONAL_CONTENT] {
        assert!(!baseline.contains(rule));
    }
    common::assert_case_deltas(common::document_feature_fixture, "baseline", CASES);
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
        let total = ValidationProfile::PdfA1b.implemented_check_count();
        assert_eq!(report.checks.total, total);
        let expected_failed = if rule_id == FILE_SPEC { 2 } else { 1 };
        assert_eq!(report.checks.failed, expected_failed);
        assert_eq!(report.checks.passed, total - expected_failed);
    }
}

#[test]
fn pdfa_2_and_3_permissions_allow_only_ur3_and_docmdp() {
    for (profile, rule_id) in [
        (ValidationProfile::PdfA2b, "PDFA2B-PERMS-ENTRIES-001"),
        (ValidationProfile::PdfA3b, "PDFA3B-PERMS-ENTRIES-001"),
    ] {
        let allowed = validate_pdf_bytes(
            &common::document_feature_fixture("permissions_allowed"),
            Some(profile),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert!(
            allowed
                .failures
                .iter()
                .all(|failure| failure.rule_id != rule_id),
            "{profile}: {allowed}"
        );
        let invalid = validate_pdf_bytes(
            &common::document_feature_fixture("permissions_invalid"),
            Some(profile),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert!(
            invalid
                .failures
                .iter()
                .any(|failure| failure.rule_id == rule_id),
            "{profile}: {invalid}"
        );
    }
}

#[test]
fn pdfa_2_and_3_reject_signature_reference_digest_keys_with_docmdp() {
    for (profile, rule_id) in [
        (ValidationProfile::PdfA2b, "PDFA2B-SIGNATURE-REFERENCE-001"),
        (ValidationProfile::PdfA3b, "PDFA3B-SIGNATURE-REFERENCE-001"),
    ] {
        let report = validate_pdf_bytes(
            &common::document_feature_fixture("signature_reference_digest"),
            Some(profile),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert!(
            report
                .failures
                .iter()
                .any(|failure| failure.rule_id == rule_id),
            "{profile}: {report}"
        );
    }
}

#[test]
fn pdfa_2_rejects_non_pdfa_embedded_files() {
    let profile = ValidationProfile::PdfA2b;
    let rule_id = "PDFA2B-EMBEDDED-FILE-PDFA-001";
    let report = validate_pdf_bytes(
        &common::document_feature_fixture("embedded_file_invalid_pdfa"),
        Some(profile),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.rule_id == rule_id),
        "{profile}: {report}"
    );
}

#[test]
fn embedded_file_pdfa_rule_matches_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let path = env::temp_dir().join(format!("page-embedded-pdfa-{}.pdf", std::process::id()));
    fs::write(
        &path,
        common::document_feature_fixture("embedded_file_invalid_pdfa"),
    )
    .expect("write embedded file fixture");
    let mut config = ReferenceConfig::pinned(&executable);
    config.profile = ReferenceProfile::PdfA2b;
    let report = DifferentialRunner::new(config)
        .expect("pinned veraPDF")
        .compare_file(&path, &SafetyLimits::default());
    assert!(
        report
            .reference_result
            .as_ref()
            .expect("veraPDF result")
            .failed_rule_ids
            .iter()
            .any(|rule| rule.to_string() == "ISO 19005-2:2011:6.8:5"),
        "{report:?}"
    );
    fs::remove_file(path).expect("remove embedded file fixture");
}

#[test]
fn permissions_key_set_matches_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let path = env::temp_dir().join(format!("page-permissions-{}.pdf", std::process::id()));
    fs::write(
        &path,
        common::document_feature_fixture("permissions_invalid"),
    )
    .expect("write permissions fixture");
    for (profile, expected_rule) in [
        (ReferenceProfile::PdfA2b, "ISO 19005-2:2011:6.1.12:1"),
        (ReferenceProfile::PdfA3b, "ISO 19005-3:2012:6.1.12:1"),
    ] {
        let mut config = ReferenceConfig::pinned(&executable);
        config.profile = profile;
        let report = DifferentialRunner::new(config)
            .expect("pinned veraPDF")
            .compare_file(&path, &SafetyLimits::default());
        assert_eq!(
            report.classification,
            ComparisonClassification::BothNoncompliant
        );
        assert!(
            report
                .reference_result
                .expect("veraPDF result")
                .failed_rule_ids
                .iter()
                .any(|rule| rule.to_string() == expected_rule),
            "{profile} did not reject the permissions key"
        );
    }
    fs::remove_file(path).expect("remove permissions fixture");
}
