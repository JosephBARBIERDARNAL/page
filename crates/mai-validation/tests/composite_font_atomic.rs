use std::collections::BTreeSet;

use mai_validation::{SafetyLimits, ValidationProfile, validate_bytes};

#[allow(dead_code)]
mod common;

const SYSTEM_INFO: &str = "PDFA1B-TYPE0-CID-SYSTEM-INFO-001";
const CID_TO_GID: &str = "PDFA1B-CIDTOGIDMAP-001";
const CMAP_EMBEDDING: &str = "PDFA1B-CMAP-EMBEDDING-001";
const CMAP_WMODE: &str = "PDFA1B-CMAP-WMODE-001";

const CASES: &[(&str, &[&str])] = &[
    ("composite_identity_v", &[]),
    ("composite_cidmap_missing", &[CID_TO_GID]),
    ("composite_cidmap_invalid_name", &[CID_TO_GID]),
    ("composite_cidmap_stream", &[]),
    ("composite_named_cmap", &[CMAP_EMBEDDING]),
    ("composite_cmap_matching", &[]),
    ("composite_cmap_mismatch_system", &[SYSTEM_INFO]),
    ("composite_cmap_wmode_match", &[]),
    ("composite_cmap_wmode_mismatch", &[CMAP_WMODE]),
];

#[test]
fn composite_font_cases_have_the_complete_expected_failure_delta() {
    let baseline = failure_ids(&common::font_fixture("composite_baseline"));
    for rule in [SYSTEM_INFO, CID_TO_GID, CMAP_EMBEDDING, CMAP_WMODE] {
        assert!(!baseline.contains(rule));
    }

    for (case, expected) in CASES {
        let actual = failure_ids(&common::font_fixture(case));
        let (added, removed) = common::rule_delta(&baseline, &actual);
        assert_eq!(
            added,
            expected
                .iter()
                .map(|rule| (*rule).to_owned())
                .collect::<BTreeSet<_>>(),
            "{case}: unexpected added failures"
        );
        assert!(
            removed.is_empty(),
            "{case}: removed baseline failures {removed:?}"
        );
    }
}

#[test]
fn a_single_composite_failure_attaches_the_type0_font() {
    let report = validate(&common::font_fixture("composite_cidmap_missing"));
    let failure = report
        .failures
        .iter()
        .find(|failure| failure.rule_id == CID_TO_GID)
        .expect("CIDToGIDMap failure");
    assert!(failure.object_id.is_some());
    assert_eq!(report.checks.total, 101);
    assert_eq!(report.checks.failed, 1);
    assert_eq!(report.checks.passed, 100);
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
