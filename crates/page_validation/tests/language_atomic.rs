pub mod common;

use std::env;

use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

const RULE: &str = "PDFA1A-LANG-001";

#[test]
fn validates_catalog_structure_and_property_list_language_values() {
    for (case, should_fail) in [
        ("lang_catalog_valid", false),
        ("lang_catalog_empty", false),
        ("lang_catalog_invalid", true),
        ("lang_catalog_overlong", true),
        ("lang_catalog_wrong_type", false),
        ("lang_catalog_null", false),
        ("lang_catalog_indirect_invalid", true),
        ("lang_structure_valid", false),
        ("lang_structure_indirect_invalid", true),
        ("lang_structure_invalid", true),
        ("lang_structure_wrong_type", false),
        ("lang_property_valid", false),
        ("lang_property_invalid", true),
        ("lang_property_indirect_invalid", true),
        ("lang_property_wrong_type", false),
        ("lang_property_null", false),
    ] {
        let report = validate_bytes_with_profile(
            &common::tagged_document_fixture(case),
            ValidationProfile::PdfA1a,
            &SafetyLimits::default(),
        );
        assert_eq!(report.checks.total, 138, "{case}");
        assert_eq!(
            report
                .failures
                .iter()
                .any(|failure| failure.rule_id == RULE),
            should_fail,
            "{case}: {:#?}",
            report.failures
        );
    }
}

#[test]
fn language_fixtures_match_pinned_verapdf() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = page_validation::differential::ReferenceConfig::pinned(executable);
    config.profile = page_validation::differential::ReferenceProfile::PdfA1a;
    let runner =
        page_validation::differential::DifferentialRunner::new(config).expect("pinned veraPDF");
    let reference_rule = "ISO 19005-1:2005:6.8.4:1";
    for case in [
        "lang_catalog_valid",
        "lang_catalog_empty",
        "lang_catalog_invalid",
        "lang_catalog_overlong",
        "lang_catalog_wrong_type",
        "lang_catalog_null",
        "lang_catalog_indirect_invalid",
        "lang_structure_valid",
        "lang_structure_indirect_invalid",
        "lang_structure_invalid",
        "lang_structure_wrong_type",
        "lang_property_valid",
        "lang_property_invalid",
        "lang_property_indirect_invalid",
        "lang_property_wrong_type",
        "lang_property_null",
    ] {
        let path = env::temp_dir().join(format!(
            "page-pdfa-1a-lang-{case}-{}.pdf",
            std::process::id()
        ));
        std::fs::write(&path, common::tagged_document_fixture(case)).expect("write fixture");
        let report = runner.compare_file(&path, &SafetyLimits::default());
        let reference = report.reference_result.as_ref().expect("veraPDF result");
        let failed = reference
            .failed_rule_ids
            .iter()
            .any(|rule| rule.to_string() == reference_rule);
        let local = validate_bytes_with_profile(
            &common::tagged_document_fixture(case),
            ValidationProfile::PdfA1a,
            &SafetyLimits::default(),
        );
        assert_eq!(
            failed,
            local.failures.iter().any(|failure| failure.rule_id == RULE),
            "{case}: {report}"
        );
        std::fs::remove_file(path).expect("remove fixture");
    }
}
