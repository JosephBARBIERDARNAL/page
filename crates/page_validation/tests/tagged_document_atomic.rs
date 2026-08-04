#[allow(dead_code)]
mod common;

use std::collections::BTreeSet;
use std::env;
use std::fs;

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

const RULE: &str = "PDFA1A-TAGGED-DOCUMENT-001";

const CASES: &[(&str, bool)] = &[
    ("tagged_valid", false),
    ("tagged_missing", true),
    ("tagged_false", true),
    ("tagged_marked_wrong_type", true),
    ("tagged_mark_info_wrong_type", true),
    ("tagged_indirect_mark_info_wrong_type", true),
    ("tagged_indirect_mark_info_null", true),
    ("tagged_indirect_mark_info", false),
    ("tagged_indirect_marked", false),
    ("tagged_struct_tree_only", true),
];

#[test]
fn tagged_document_cases_enforce_catalog_mark_info() {
    for (case, should_fail) in CASES {
        let report = validate_bytes_with_profile(
            &common::tagged_document_fixture(case),
            ValidationProfile::PdfA1a,
            &SafetyLimits::default(),
        );
        assert_eq!(report.checks.total, 135, "{case}");
        assert_eq!(
            report
                .failures
                .iter()
                .any(|failure| failure.rule_id == RULE),
            *should_fail,
            "{case}: {:#?}",
            report.failures
        );
    }
}

#[test]
fn tagged_document_cases_match_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfA1a;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    let reference_rule = "ISO 19005-1:2005:6.8.2.2:1";

    for (case, should_fail) in CASES {
        let path = env::temp_dir().join(format!(
            "page-pdfa-1a-tagged-document-{case}-{}.pdf",
            std::process::id()
        ));
        fs::write(&path, common::tagged_document_fixture(case)).expect("write fixture");
        let report = runner.compare_file(&path, &SafetyLimits::default());
        let reference = report.reference_result.as_ref().expect("veraPDF result");
        let failed = reference
            .failed_rule_ids
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            failed.contains(reference_rule),
            *should_fail,
            "{case}: {report}"
        );
        fs::remove_file(path).expect("remove fixture");
    }
}
