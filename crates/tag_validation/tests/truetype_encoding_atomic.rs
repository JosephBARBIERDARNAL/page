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
    ("tt_nonsymbolic_differences_null", &[]),
    ("tt_symbolic_no_encoding", &[]),
    ("tt_symbolic_indirect_flags", &[]),
    ("tt_symbolic_with_encoding", &[SYMBOLIC]),
    ("tt_symbolic_one_cmap", &[]),
    ("tt_symbolic_two_cmaps", &[SYMBOLIC_CMAP]),
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
