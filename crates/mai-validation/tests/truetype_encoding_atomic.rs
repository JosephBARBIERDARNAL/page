use std::collections::BTreeSet;

use mai_validation::{SafetyLimits, ValidationProfile, validate_bytes};

#[allow(dead_code)]
mod common;

const NONSYMBOLIC: &str = "PDFA1B-TRUETYPE-NONSYMBOLIC-ENCODING-001";
const SYMBOLIC: &str = "PDFA1B-TRUETYPE-SYMBOLIC-ENCODING-001";
const SYMBOLIC_CMAP: &str = "PDFA1B-TRUETYPE-SYMBOLIC-CMAP-001";

const CASES: &[(&str, &[&str])] = &[
    ("tt_nonsymbolic_macroman", &[]),
    ("tt_nonsymbolic_missing_encoding", &[NONSYMBOLIC]),
    ("tt_nonsymbolic_invalid_encoding", &[NONSYMBOLIC]),
    ("tt_nonsymbolic_dictionary_winansi", &[]),
    ("tt_nonsymbolic_dictionary_macroman", &[]),
    ("tt_nonsymbolic_differences", &[NONSYMBOLIC]),
    ("tt_symbolic_no_encoding", &[]),
    ("tt_symbolic_with_encoding", &[SYMBOLIC]),
    ("tt_symbolic_one_cmap", &[]),
    ("tt_symbolic_two_cmaps", &[SYMBOLIC_CMAP]),
];

#[test]
fn truetype_encoding_cases_have_the_complete_expected_failure_delta() {
    let baseline = failure_ids(&common::font_fixture("baseline_embedded"));
    for rule in [NONSYMBOLIC, SYMBOLIC, SYMBOLIC_CMAP] {
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
fn symbolic_cmap_failure_reports_the_table_count() {
    let report = validate(&common::font_fixture("tt_symbolic_two_cmaps"));
    let failure = report
        .failures
        .iter()
        .find(|failure| failure.rule_id == SYMBOLIC_CMAP)
        .expect("symbolic cmap failure");
    assert!(failure.message.contains("2 cmap subtables"));
    assert_eq!(report.checks.total, 109);
    assert_eq!(report.checks.failed, 1);
    assert_eq!(report.checks.passed, 108);
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
