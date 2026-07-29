use std::collections::BTreeSet;

#[allow(dead_code)]
mod common;

const SYSTEM_INFO: &str = "PDFA1B-TYPE0-CID-SYSTEM-INFO-001";
const CID_TO_GID: &str = "PDFA1B-CIDTOGIDMAP-001";
const CMAP_EMBEDDING: &str = "PDFA1B-CMAP-EMBEDDING-001";
const CMAP_WMODE: &str = "PDFA1B-CMAP-WMODE-001";
const CMAP_CID_RANGE: &str = "PDFA1B-CMAP-CID-RANGE-001";
const CID_SUBSET_CIDSET: &str = "PDFA1B-CID-SUBSET-CIDSET-001";

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
    ("composite_cmap_cid_too_large", &[CMAP_CID_RANGE]),
    ("composite_cid_subset_missing_cidset", &[CID_SUBSET_CIDSET]),
];

#[test]
fn composite_font_cases_have_the_complete_expected_failure_delta() {
    let baseline = common::failure_ids(&common::font_fixture("composite_baseline"));
    for rule in [
        SYSTEM_INFO,
        CID_TO_GID,
        CMAP_EMBEDDING,
        CMAP_WMODE,
        CMAP_CID_RANGE,
        CID_SUBSET_CIDSET,
    ] {
        assert!(!baseline.contains(rule));
    }

    for (case, expected) in CASES {
        let actual = common::failure_ids(&common::font_fixture(case));
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
    let report = common::validate(&common::font_fixture("composite_cidmap_missing"));
    let failure = common::assert_single_failure(&report, CID_TO_GID);
    assert!(failure.object_id.is_some());
}
