use std::collections::BTreeSet;

use mai_validation::{SafetyLimits, ValidationProfile, validate_bytes};

#[allow(dead_code)]
mod common;

const TYPE: &str = "PDFA1B-FONT-TYPE-001";
const SUBTYPE: &str = "PDFA1B-FONT-SUBTYPE-001";
const BASE_FONT: &str = "PDFA1B-FONT-BASEFONT-001";
const FIRST_CHAR: &str = "PDFA1B-FONT-FIRSTCHAR-001";
const LAST_CHAR: &str = "PDFA1B-FONT-LASTCHAR-001";
const WIDTHS: &str = "PDFA1B-FONT-WIDTHS-001";
const FILE_SUBTYPE: &str = "PDFA1B-FONT-FILE-SUBTYPE-001";
const EMBEDDING: &str = "PDFA1B-FONT-EMBEDDING-001";

const CASES: &[(&str, &[&str])] = &[
    ("font_type_missing", &[TYPE]),
    ("font_type_invalid", &[TYPE]),
    ("font_subtype_missing", &[]),
    ("font_subtype_invalid", &[]),
    ("font_basefont_missing", &[BASE_FONT]),
    ("font_basefont_invalid", &[BASE_FONT]),
    ("font_firstchar_missing", &[FIRST_CHAR, WIDTHS]),
    ("font_lastchar_missing", &[LAST_CHAR, WIDTHS]),
    ("font_widths_missing", &[WIDTHS]),
    ("font_widths_wrong_size", &[WIDTHS]),
    ("standard14_missing_metrics", &[EMBEDDING]),
    ("font_file_subtype_invalid", &[FILE_SUBTYPE]),
    ("unused_invalid_font", &[]),
    ("type3_visible", &[]),
];

#[test]
fn font_dictionary_cases_have_the_complete_expected_failure_delta() {
    let baseline = failure_ids(&common::font_fixture("baseline_embedded"));
    for rule in [
        TYPE,
        SUBTYPE,
        BASE_FONT,
        FIRST_CHAR,
        LAST_CHAR,
        WIDTHS,
        FILE_SUBTYPE,
    ] {
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
fn a_single_invalid_font_attaches_the_font_object() {
    let report = validate(&common::font_fixture("font_basefont_missing"));
    let failure = report
        .failures
        .iter()
        .find(|failure| failure.rule_id == BASE_FONT)
        .expect("BaseFont failure");
    assert!(failure.object_id.is_some());
    assert_eq!(report.checks.total, 108);
    assert_eq!(report.checks.failed, 1);
    assert_eq!(report.checks.passed, 107);
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
