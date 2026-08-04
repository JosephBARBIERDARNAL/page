pub mod common;

const INTEGER: &str = "PDFA1B-INTEGER-RANGE-001";
const STRING: &str = "PDFA1B-STRING-LENGTH-001";
const NAME: &str = "PDFA1B-NAME-LENGTH-001";
const HEX_LENGTH: &str = "PDFA1B-HEX-STRING-LENGTH-001";
const HEX_CHARACTERS: &str = "PDFA1B-HEX-STRING-CHARACTERS-001";
const TRAILER_ID: &str = "PDFA1B-TRAILER-ID-001";
const STREAM_LENGTH: &str = "PDFA1B-STREAM-LENGTH-001";
const STREAM_EXTERNAL: &str = "PDFA1B-STREAM-EXTERNAL-DATA-001";
const STREAM_LZW: &str = "PDFA1B-STREAM-LZW-001";

const CASES: &[(&str, &[&str])] = &[
    ("baseline", &[]),
    ("duplicate_last_null", &[]),
    ("duplicate_last_invalid", &[INTEGER]),
    ("escaped_name_at_boundary", &[]),
    ("escaped_name_over_boundary", &[NAME]),
    ("literal_string_at_boundary", &[]),
    ("literal_string_over_boundary", &[STRING]),
    ("hex_string_invalid_character", &[HEX_CHARACTERS]),
    ("hex_string_odd", &[HEX_LENGTH]),
    ("incremental_stale_invalid", &[]),
    ("incremental_active_invalid", &[INTEGER]),
    ("empty_trailer_id", &[]),
    ("single_trailer_id", &[]),
    ("wrong_type_trailer_id", &[TRAILER_ID]),
    ("stream_direct_length_valid", &[]),
    ("stream_direct_length_mismatch", &[STREAM_LENGTH]),
    ("stream_indirect_length_valid", &[]),
    ("stream_indirect_length_mismatch", &[STREAM_LENGTH]),
    ("stream_duplicate_length_last_valid", &[]),
    ("stream_duplicate_length_last_invalid", &[STREAM_LENGTH]),
    ("stream_duplicate_external_last_null", &[]),
    ("stream_duplicate_external_last_invalid", &[STREAM_EXTERNAL]),
    ("stream_escaped_lzw_filter", &[STREAM_LZW]),
];

#[test]
fn raw_syntax_cases_have_the_complete_expected_failure_delta() {
    common::assert_case_deltas(common::syntax_fixture, "baseline", CASES);
}

#[test]
fn recoverable_hex_syntax_is_a_conformance_failure_not_a_parser_failure() {
    let report = common::validate(&common::syntax_fixture("hex_string_invalid_character"));
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.rule_id == HEX_CHARACTERS)
    );
    assert!(
        report
            .failures
            .iter()
            .all(|failure| failure.rule_id != "PDF-PARSE-001")
    );
}
