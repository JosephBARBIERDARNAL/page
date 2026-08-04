pub mod common;

use std::{env, fs};

use page_validation::differential::{
    ComparisonClassification, DifferentialRunner, ReferenceConfig, ReferenceProfile,
};
use page_validation::{
    FailureCategory, SafetyLimits, ValidationProfile, validate_bytes_with_profile,
};

const RULE: &str = "PDFA1A-STRUCT-TREE-ROOT-001";

#[test]
fn structure_tree_root_cases_are_distinguished() {
    for (case, should_fail) in [
        ("baseline", true),
        ("struct_tree_missing", true),
        ("struct_tree_direct_valid", false),
        ("struct_tree_minimal_valid", false),
        ("struct_tree_indirect_valid", false),
        ("struct_tree_invalid", true),
        ("struct_tree_indirect_invalid", true),
        ("struct_tree_unsupported_shape", false),
        ("struct_tree_parent_child", false),
    ] {
        let report = validate_bytes_with_profile(
            &common::tagged_document_fixture(case),
            ValidationProfile::PdfA1a,
            &SafetyLimits::default(),
        );
        assert_eq!(report.checks.total, 136, "{case}");
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
fn cyclic_structure_tree_is_an_operational_failure() {
    let report = validate_bytes_with_profile(
        &common::tagged_document_fixture("struct_tree_cyclic"),
        ValidationProfile::PdfA1a,
        &SafetyLimits::default(),
    );
    assert_eq!(report.exit_code(), 1);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].rule_id, "RESOURCE-LIMIT-001");
    assert_eq!(report.failures[0].category, FailureCategory::Operational);
}

#[test]
fn structure_tree_fixtures_match_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfA1a;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    let reference_rule = "ISO 19005-1:2005:6.8.3.3:1";

    for (case, should_fail) in [
        ("baseline", true),
        ("struct_tree_missing", true),
        ("struct_tree_direct_valid", false),
        ("struct_tree_minimal_valid", false),
        ("struct_tree_indirect_valid", false),
        ("struct_tree_invalid", true),
        ("struct_tree_indirect_invalid", true),
        ("struct_tree_unsupported_shape", false),
        ("struct_tree_parent_child", false),
    ] {
        let path = env::temp_dir().join(format!(
            "page-pdfa-1a-struct-tree-{case}-{}.pdf",
            std::process::id()
        ));
        fs::write(&path, common::tagged_document_fixture(case)).expect("write fixture");
        let report = runner.compare_file(&path, &SafetyLimits::default());
        let reference = report.reference_result.as_ref().expect("veraPDF result");
        let failed = reference
            .failed_rule_ids
            .iter()
            .any(|rule| rule.to_string() == reference_rule);
        assert_eq!(failed, should_fail, "{case}: {report}");
        fs::remove_file(path).expect("remove fixture");
    }

    let path = env::temp_dir().join(format!(
        "page-pdfa-1a-struct-tree-cyclic-{}.pdf",
        std::process::id()
    ));
    fs::write(&path, common::tagged_document_fixture("struct_tree_cyclic")).expect("write fixture");
    let report = runner.compare_file(&path, &SafetyLimits::default());
    assert_eq!(report.classification, ComparisonClassification::Operational);
    fs::remove_file(path).expect("remove fixture");
}
