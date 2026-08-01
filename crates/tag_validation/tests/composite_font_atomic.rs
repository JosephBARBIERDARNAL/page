use std::{env, fs};

use tag_validation::SafetyLimits;
use tag_validation::differential::{ComparisonClassification, DifferentialRunner, ReferenceConfig};

#[allow(dead_code)]
mod common;

const SYSTEM_INFO: &str = "PDFA1B-TYPE0-CID-SYSTEM-INFO-001";
const CID_TO_GID: &str = "PDFA1B-CIDTOGIDMAP-001";
const CMAP_EMBEDDING: &str = "PDFA1B-CMAP-EMBEDDING-001";
const CMAP_WMODE: &str = "PDFA1B-CMAP-WMODE-001";
const CMAP_CID_RANGE: &str = "PDFA1B-CMAP-CID-RANGE-001";
const CMAP_MAX_CID: &str = "PDFA1B-CMAP-MAX-CID-001";
const CID_SUBSET_CIDSET: &str = "PDFA1B-CID-SUBSET-CIDSET-001";
const GLYPH_PRESENCE: &str = "PDFA1B-TRUETYPE-GLYPH-PRESENCE-001";
const GLYPH_WIDTH: &str = "PDFA1B-TRUETYPE-GLYPH-WIDTH-001";

const CASES: &[(&str, &[&str])] = &[
    ("composite_identity_v", &[]),
    ("composite_indirect_identity_h", &[]),
    ("composite_cidmap_missing", &[CID_TO_GID]),
    ("composite_cidmap_missing_indirect_subtype", &[CID_TO_GID]),
    ("composite_cidmap_invalid_name", &[CID_TO_GID]),
    ("composite_cidmap_stream", &[]),
    ("composite_cidmap_indirect_identity", &[]),
    ("composite_named_cmap", &[CMAP_EMBEDDING]),
    ("composite_cmap_matching", &[GLYPH_PRESENCE]),
    ("composite_indirect_cid_system_info", &[GLYPH_PRESENCE]),
    (
        "composite_cmap_mismatch_system",
        &[SYSTEM_INFO, GLYPH_PRESENCE],
    ),
    ("composite_cmap_wmode_match", &[GLYPH_PRESENCE]),
    (
        "composite_cmap_wmode_mismatch",
        &[CMAP_WMODE, GLYPH_PRESENCE],
    ),
    ("composite_cmap_wmode_indirect_match", &[GLYPH_PRESENCE]),
    (
        "composite_cmap_cid_too_large",
        &[CMAP_CID_RANGE, CMAP_MAX_CID],
    ),
    ("composite_cid_subset_missing_cidset", &[CID_SUBSET_CIDSET]),
    ("composite_cidset_real_program", &[CID_SUBSET_CIDSET]),
    (
        "composite_cidset_indirect_basefont_real_program",
        &[CID_SUBSET_CIDSET],
    ),
    (
        "composite_cidset_nonidentity_real_program",
        &[CID_SUBSET_CIDSET],
    ),
    ("composite_identity_missing_glyph", &[GLYPH_PRESENCE]),
    ("composite_identity_width_mismatch", &[GLYPH_WIDTH]),
    (
        "composite_descendant_subtype_indirect_width_mismatch",
        &[GLYPH_WIDTH],
    ),
    ("composite_identity_width_override_mismatch", &[GLYPH_WIDTH]),
    ("composite_dw_indirect_mismatch", &[GLYPH_WIDTH]),
    (
        "composite_w_singles_element_indirect_mismatch",
        &[GLYPH_WIDTH],
    ),
    ("composite_stream_cidmap_missing_glyph", &[GLYPH_PRESENCE]),
    ("composite_nonidentity_missing_glyph", &[GLYPH_PRESENCE]),
    (
        "composite_nonidentity_multibyte_missing_glyph",
        &[GLYPH_PRESENCE],
    ),
    (
        "composite_identity_usecmap_missing_glyph",
        &[GLYPH_PRESENCE],
    ),
    ("composite_cff_missing_glyph", &[GLYPH_PRESENCE]),
    ("composite_cff_present_glyph", &[]),
    ("composite_cff_width_mismatch", &[GLYPH_WIDTH]),
    ("composite_cff_cidset_missing", &[CID_SUBSET_CIDSET]),
];

#[test]
fn composite_font_cases_have_the_complete_expected_failure_delta() {
    let missing_cff_bytes = common::minimal_cidfonttype0c(false);
    let missing_cff = ttf_parser::cff::Table::parse(&missing_cff_bytes).expect("parse CID CFF");
    assert!(
        !(0..missing_cff.number_of_glyphs())
            .map(ttf_parser::GlyphId)
            .any(|glyph| missing_cff.glyph_cid(glyph) == Some(32))
    );
    let present_cff_bytes = common::minimal_cidfonttype0c(true);
    let present_cff = ttf_parser::cff::Table::parse(&present_cff_bytes).expect("parse CID CFF");
    assert!(
        (0..present_cff.number_of_glyphs())
            .map(ttf_parser::GlyphId)
            .any(|glyph| present_cff.glyph_cid(glyph) == Some(32))
    );
    let baseline = common::failure_ids(&common::font_fixture("composite_baseline"));
    for rule in [
        SYSTEM_INFO,
        CID_TO_GID,
        CMAP_EMBEDDING,
        CMAP_WMODE,
        CMAP_CID_RANGE,
        CMAP_MAX_CID,
        CID_SUBSET_CIDSET,
        GLYPH_PRESENCE,
        GLYPH_WIDTH,
    ] {
        assert!(!baseline.contains(rule));
    }

    common::assert_case_deltas(common::font_fixture, "composite_baseline", CASES);
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
        let path = env::temp_dir().join(format!("tag-{case}-{}.pdf", std::process::id()));
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
            "composite_nonidentity_multibyte_missing_glyph",
            GLYPH_PRESENCE,
            "ISO 19005-1:2005:6.3.5:1",
        ),
        (
            "composite_identity_usecmap_missing_glyph",
            GLYPH_PRESENCE,
            "ISO 19005-1:2005:6.3.5:1",
        ),
        (
            "composite_cff_missing_glyph",
            GLYPH_PRESENCE,
            "ISO 19005-1:2005:6.3.5:1",
        ),
        (
            "composite_cff_width_mismatch",
            GLYPH_WIDTH,
            "ISO 19005-1:2005:6.3.6:1",
        ),
        (
            "composite_cff_cidset_missing",
            CID_SUBSET_CIDSET,
            "ISO 19005-1:2005:6.3.5:3",
        ),
        (
            "composite_cmap_cid_too_large",
            CMAP_MAX_CID,
            "ISO 19005-1:2005:6.1.12:10",
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
        let path = env::temp_dir().join(format!("tag-{case}-{}.pdf", std::process::id()));
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
