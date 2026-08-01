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
const TYPE1_SUBSET_CHARSET: &str = "PDFA1B-TYPE1-SUBSET-CHARSET-001";
const TYPE1_GLYPH_PRESENCE: &str = "PDFA1B-TYPE1-GLYPH-PRESENCE-001";
const GLYPH_WIDTH: &str = "PDFA1B-TRUETYPE-GLYPH-WIDTH-001";

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
    ("font_widths_array_indirect", &[]),
    ("font_widths_element_indirect_mismatch", &[GLYPH_WIDTH]),
    ("font_firstchar_lastchar_indirect", &[]),
    ("standard14_missing_metrics", &[EMBEDDING]),
    (
        "truetype_named_standard14_missing_metrics",
        &[FIRST_CHAR, LAST_CHAR, WIDTHS, EMBEDDING],
    ),
    ("font_file_subtype_invalid", &[FILE_SUBTYPE]),
    ("type1_subset_missing_charset", &[TYPE1_SUBSET_CHARSET]),
    ("unused_invalid_font", &[]),
    ("type3_visible", &[TYPE1_GLYPH_PRESENCE, GLYPH_WIDTH]),
];

#[test]
fn font_dictionary_cases_have_the_complete_expected_failure_delta() {
    let baseline = common::failure_ids(&common::font_fixture("baseline_embedded"));
    for rule in [
        TYPE,
        SUBTYPE,
        BASE_FONT,
        FIRST_CHAR,
        LAST_CHAR,
        WIDTHS,
        FILE_SUBTYPE,
        TYPE1_SUBSET_CHARSET,
    ] {
        assert!(!baseline.contains(rule));
    }

    common::assert_case_deltas(common::font_fixture, "baseline_embedded", CASES);
}

#[test]
fn a_single_invalid_font_attaches_the_font_object() {
    let report = common::validate(&common::font_fixture("font_basefont_missing"));
    let failure = common::assert_single_failure(&report, BASE_FONT);
    assert!(failure.object_id.is_some());
}
