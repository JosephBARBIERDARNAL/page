pub mod common;

const HEX_CHARACTERS: &str = "PDFA1B-HEX-STRING-CHARACTERS-001";

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
