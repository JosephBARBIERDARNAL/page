use std::collections::BTreeSet;
use std::{env, fs};

use mai_validation::SafetyLimits;
use mai_validation::differential::{ComparisonClassification, DifferentialRunner, ReferenceConfig};

#[allow(dead_code)]
mod common;

const SYSTEM_INFO: &str = "PDFA1B-TYPE0-CID-SYSTEM-INFO-001";
const CID_TO_GID: &str = "PDFA1B-CIDTOGIDMAP-001";
const CMAP_EMBEDDING: &str = "PDFA1B-CMAP-EMBEDDING-001";
const CMAP_WMODE: &str = "PDFA1B-CMAP-WMODE-001";
const CMAP_CID_RANGE: &str = "PDFA1B-CMAP-CID-RANGE-001";
const CID_SUBSET_CIDSET: &str = "PDFA1B-CID-SUBSET-CIDSET-001";
const GLYPH_PRESENCE: &str = "PDFA1B-TRUETYPE-GLYPH-PRESENCE-001";
const GLYPH_WIDTH: &str = "PDFA1B-TRUETYPE-GLYPH-WIDTH-001";

const CASES: &[(&str, &[&str])] = &[
    ("composite_identity_v", &[]),
    ("composite_cidmap_missing", &[CID_TO_GID]),
    ("composite_cidmap_invalid_name", &[CID_TO_GID]),
    ("composite_cidmap_stream", &[]),
    ("composite_named_cmap", &[CMAP_EMBEDDING]),
    ("composite_cmap_matching", &[GLYPH_PRESENCE]),
    (
        "composite_cmap_mismatch_system",
        &[SYSTEM_INFO, GLYPH_PRESENCE],
    ),
    ("composite_cmap_wmode_match", &[GLYPH_PRESENCE]),
    (
        "composite_cmap_wmode_mismatch",
        &[CMAP_WMODE, GLYPH_PRESENCE],
    ),
    ("composite_cmap_cid_too_large", &[CMAP_CID_RANGE]),
    ("composite_cid_subset_missing_cidset", &[CID_SUBSET_CIDSET]),
    ("composite_cidset_real_program", &[CID_SUBSET_CIDSET]),
    (
        "composite_cidset_nonidentity_real_program",
        &[CID_SUBSET_CIDSET],
    ),
    ("composite_identity_missing_glyph", &[GLYPH_PRESENCE]),
    ("composite_identity_width_mismatch", &[GLYPH_WIDTH]),
    ("composite_identity_width_override_mismatch", &[GLYPH_WIDTH]),
    ("composite_stream_cidmap_missing_glyph", &[GLYPH_PRESENCE]),
    ("composite_nonidentity_missing_glyph", &[GLYPH_PRESENCE]),
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
        GLYPH_PRESENCE,
        GLYPH_WIDTH,
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

#[test]
fn rendered_cidset_coverage_matches_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let runner = DifferentialRunner::new(ReferenceConfig::pinned(executable)).expect("veraPDF");
    for case in [
        "composite_cidset_real_program",
        "composite_cidset_nonidentity_real_program",
    ] {
        let path = env::temp_dir().join(format!("mai-{case}-{}.pdf", std::process::id()));
        fs::write(&path, common::font_fixture(case)).expect("write CIDSet fixture");
        let report = runner.compare_file(&path, &SafetyLimits::default());
        assert_eq!(
            report.classification,
            ComparisonClassification::BothNoncompliant,
            "{case}"
        );
        assert!(
            common::failure_ids(&fs::read(&path).expect("read CIDSet fixture"))
                .contains(CID_SUBSET_CIDSET),
            "{case}"
        );
        let reference = report.reference_result.expect("veraPDF result");
        assert!(
            reference
                .failed_rule_ids
                .iter()
                .map(ToString::to_string)
                .any(|rule| rule == "ISO 19005-1:2005:6.3.5:3"),
            "{case}"
        );
        fs::remove_file(path).expect("remove CIDSet fixture");
    }
}

#[test]
fn rendered_identity_cidfont_program_checks_match_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let runner = DifferentialRunner::new(ReferenceConfig::pinned(executable)).expect("veraPDF");
    for (case, local_rule, reference_rule) in [
        (
            "composite_identity_missing_glyph",
            GLYPH_PRESENCE,
            "ISO 19005-1:2005:6.3.5:1",
        ),
        (
            "composite_identity_width_mismatch",
            GLYPH_WIDTH,
            "ISO 19005-1:2005:6.3.6:1",
        ),
        (
            "composite_identity_width_override_mismatch",
            GLYPH_WIDTH,
            "ISO 19005-1:2005:6.3.6:1",
        ),
        (
            "composite_stream_cidmap_missing_glyph",
            GLYPH_PRESENCE,
            "ISO 19005-1:2005:6.3.5:1",
        ),
        (
            "composite_nonidentity_missing_glyph",
            GLYPH_PRESENCE,
            "ISO 19005-1:2005:6.3.5:1",
        ),
        (
            "composite_cmap_matching",
            GLYPH_PRESENCE,
            "ISO 19005-1:2005:6.3.5:1",
        ),
        (
            "composite_cmap_wmode_match",
            GLYPH_PRESENCE,
            "ISO 19005-1:2005:6.3.5:1",
        ),
    ] {
        let path = env::temp_dir().join(format!("mai-{case}-{}.pdf", std::process::id()));
        fs::write(&path, common::font_fixture(case)).expect("write CIDFont fixture");
        let report = runner.compare_file(&path, &SafetyLimits::default());
        assert_eq!(
            report.classification,
            ComparisonClassification::BothNoncompliant,
            "{case}"
        );
        assert!(
            common::failure_ids(&fs::read(&path).expect("read CIDFont fixture"))
                .contains(local_rule),
            "{case}"
        );
        let reference = report.reference_result.expect("veraPDF result");
        assert!(
            reference
                .failed_rule_ids
                .iter()
                .map(ToString::to_string)
                .any(|rule| rule == reference_rule),
            "{case}"
        );
        fs::remove_file(path).expect("remove CIDFont fixture");
    }
}
