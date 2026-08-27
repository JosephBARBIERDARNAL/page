use std::{env, fs};

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes};

pub mod common;

const NONSYMBOLIC: &str = "PDFA1B-TRUETYPE-NONSYMBOLIC-ENCODING-001";
const SYMBOLIC: &str = "PDFA1B-TRUETYPE-SYMBOLIC-ENCODING-001";
const SYMBOLIC_CMAP: &str = "PDFA1B-TRUETYPE-SYMBOLIC-CMAP-001";

const CASES: &[(&str, &[&str])] = &[
    ("tt_nonsymbolic_macroman", &[]),
    ("tt_nonsymbolic_missing_encoding", &[NONSYMBOLIC]),
    ("tt_nonsymbolic_invalid_encoding", &[NONSYMBOLIC]),
    ("tt_nonsymbolic_dictionary_winansi", &[]),
    ("tt_nonsymbolic_dictionary_macroman", &[]),
    ("tt_nonsymbolic_dictionary_indirect_baseencoding", &[]),
    ("tt_nonsymbolic_differences", &[NONSYMBOLIC]),
    ("tt_nonsymbolic_differences_null", &[]),
    (
        "tt_nonsymbolic_zero_cmaps",
        &[NONSYMBOLIC, "PDFA1B-TRUETYPE-GLYPH-PRESENCE-001"],
    ),
    (
        "tt_nonsymbolic_one_cmap30",
        &[NONSYMBOLIC, "PDFA1B-TRUETYPE-GLYPH-PRESENCE-001"],
    ),
    ("tt_symbolic_no_encoding", &[]),
    ("tt_symbolic_indirect_flags", &[]),
    ("tt_symbolic_with_encoding", &[SYMBOLIC]),
    ("tt_symbolic_one_cmap", &[]),
    ("tt_symbolic_two_cmaps", &[SYMBOLIC_CMAP]),
    ("tt_symbolic_two_cmaps_with_cmap30", &[]),
];

#[test]
fn truetype_encoding_cases_have_the_complete_expected_failure_delta() {
    let baseline = common::failure_ids(&common::font_fixture("baseline_embedded"));
    for rule in [NONSYMBOLIC, SYMBOLIC, SYMBOLIC_CMAP] {
        assert!(!baseline.contains(rule));
    }

    common::assert_case_deltas(common::font_fixture, "baseline_embedded", CASES);
}

#[test]
fn symbolic_cmap_failure_reports_the_table_count() {
    let report = common::validate(&common::font_fixture("tt_symbolic_two_cmaps"));
    let failure = common::assert_single_failure(&report, SYMBOLIC_CMAP);
    assert!(failure.message.contains("2 cmap subtables"));
}

#[test]
fn symbolic_cmap_predicate_matches_pdfa_2_and_3_profiles() {
    for (profile, rule) in [
        (
            ValidationProfile::PdfA2b,
            "PDFA2B-TRUETYPE-SYMBOLIC-CMAP-001",
        ),
        (
            ValidationProfile::PdfA3b,
            "PDFA3B-TRUETYPE-SYMBOLIC-CMAP-001",
        ),
    ] {
        let invalid = validate_bytes(
            &common::font_fixture("tt_symbolic_two_cmaps"),
            Some(profile),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert!(
            invalid
                .failures
                .iter()
                .any(|failure| failure.rule_id == rule)
        );
        let valid = validate_bytes(
            &common::font_fixture("tt_symbolic_two_cmaps_with_cmap30"),
            Some(profile),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert!(valid.failures.iter().all(|failure| failure.rule_id != rule));
    }
}

#[test]
fn symbolic_cmap_predicate_matches_pinned_verapdf_for_pdfa_2_and_3() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let path = env::temp_dir().join(format!("page-symbolic-cmap-{}.pdf", std::process::id()));
    for case in ["tt_symbolic_two_cmaps", "tt_symbolic_two_cmaps_with_cmap30"] {
        fs::write(&path, common::font_fixture(case)).expect("write symbolic cmap fixture");
        for (profile, rule) in [
            (ReferenceProfile::PdfA2b, "ISO 19005-2:2011:6.2.11.6:4"),
            (ReferenceProfile::PdfA3b, "ISO 19005-3:2012:6.2.11.6:4"),
        ] {
            let mut config = ReferenceConfig::pinned(&executable);
            config.profile = profile;
            let report = DifferentialRunner::new(config)
                .expect("pinned veraPDF")
                .compare_file(&path, &SafetyLimits::default());
            let reference = report
                .reference_result
                .as_ref()
                .unwrap_or_else(|| panic!("reference result: {report}"));
            assert_eq!(
                reference
                    .failed_rule_ids
                    .iter()
                    .any(|id| id.to_string() == rule),
                case == "tt_symbolic_two_cmaps",
                "{case} {profile}: {report}"
            );
            assert!(
                report.operational_failure.is_none(),
                "{case} {profile}: {report}"
            );
        }
    }
    fs::remove_file(path).expect("remove symbolic cmap fixture");
}

#[test]
fn nonsymbolic_cmap_predicate_matches_pinned_verapdf_for_pdfa_2_and_3() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let path = env::temp_dir().join(format!("page-nonsymbolic-cmap-{}.pdf", std::process::id()));
    for case in ["tt_nonsymbolic_zero_cmaps", "tt_nonsymbolic_one_cmap30"] {
        fs::write(&path, common::font_fixture(case)).expect("write nonsymbolic cmap fixture");
        for (profile, rule) in [
            (ReferenceProfile::PdfA2b, "ISO 19005-2:2011:6.2.11.6:1"),
            (ReferenceProfile::PdfA3b, "ISO 19005-3:2012:6.2.11.6:1"),
        ] {
            let mut config = ReferenceConfig::pinned(&executable);
            config.profile = profile;
            let report = DifferentialRunner::new(config)
                .expect("pinned veraPDF")
                .compare_file(&path, &SafetyLimits::default());
            let reference = report
                .reference_result
                .as_ref()
                .unwrap_or_else(|| panic!("reference result: {report}"));
            assert!(
                reference
                    .failed_rule_ids
                    .iter()
                    .any(|id| id.to_string() == rule),
                "{case} {profile}: {report}"
            );
            assert!(
                report.operational_failure.is_none(),
                "{case} {profile}: {report}"
            );
        }
    }
    fs::remove_file(path).expect("remove nonsymbolic cmap fixture");
}

/// Confirmed live against veraPDF 1.30.2 via reprex: a TrueType font's
/// `/Encoding` present as a value that is neither a name, a dictionary, nor
/// null (a `Boolean` here) crashes veraPDF's own validation entirely --
/// `Wrapped java.lang.NullPointerException: Cannot invoke
/// "org.verapdf.cos.COSObject.getString()" because the return value of
/// "org.verapdf.cos.COSObject.getKey(org.verapdf.as.ASAtom)" is null` --
/// for both a symbolic and a non-symbolic font (the crash is unconditional,
/// not gated on the Symbolic flag). This is a genuine upstream veraPDF
/// robustness bug, not a local gap: no differential result exists to match
/// for this exact shape. This test only pins that the local implementation
/// itself stays bounded (no panic) and produces a defined result, without
/// asserting which specific rule fires, since that answer cannot be
/// verified against veraPDF for this input.
#[test]
fn malformed_encoding_type_does_not_panic_locally() {
    let _ = common::validate(&common::font_fixture("tt_symbolic_malformed_encoding"));
    let _ = common::validate(&common::font_fixture("tt_nonsymbolic_malformed_encoding"));
}
