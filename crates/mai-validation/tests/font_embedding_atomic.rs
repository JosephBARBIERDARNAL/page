use std::collections::BTreeSet;
use std::{env, fs};

use mai_validation::differential::{ComparisonClassification, DifferentialRunner, ReferenceConfig};
use mai_validation::{PdfDocument, SafetyLimits, ValidationProfile, validate_bytes};

#[allow(dead_code)]
mod common;

const RULE: &str = "PDFA1B-FONT-EMBEDDING-001";
const TYPE1_GLYPH_PRESENCE: &str = "PDFA1B-TYPE1-GLYPH-PRESENCE-001";
const TYPE1_SUBSET_CHARSET: &str = "PDFA1B-TYPE1-SUBSET-CHARSET-001";
const GLYPH_WIDTH: &str = "PDFA1B-TRUETYPE-GLYPH-WIDTH-001";

const CASES: &[(&str, bool)] = &[
    ("unembedded_visible", true),
    ("unembedded_invisible", false),
    ("mixed_rendering_modes", false),
    ("mixed_visible_first", true),
    ("unused_resource", false),
    ("selected_not_shown", false),
    ("direct_font", true),
    ("form_unembedded", true),
    ("nested_form_unembedded", true),
    ("inherited_resources", true),
    ("type3_visible", false),
    ("type0_unembedded_descendant", true),
    ("type0_embedded_descendant", false),
    ("missing_descriptor", true),
    ("malformed_descriptor", true),
    ("malformed_font_program", true),
    ("malformed_font_file", true),
    ("missing_font_file_object", true),
    ("direct_descriptor", false),
    ("direct_font_file", false),
    ("repeated_aliases", true),
    ("two_unembedded_fonts", true),
    ("graphics_state_visible", true),
    ("graphics_state_invisible", false),
    ("cyclic_form", true),
];

#[test]
fn font_cases_have_the_complete_expected_failure_delta() {
    let baseline = common::failure_ids(&common::font_fixture("baseline_embedded"));
    assert!(!baseline.contains(RULE));
    for (case, should_fail) in CASES {
        let actual = common::failure_ids(&common::font_fixture(case));
        let (added, removed) = common::rule_delta(&baseline, &actual);
        let expected = if *should_fail {
            BTreeSet::from([RULE.to_owned()])
        } else {
            BTreeSet::new()
        };
        assert_eq!(added, expected, "{case}: unexpected added failures");
        assert!(
            removed.is_empty(),
            "{case}: removed baseline failures {removed:?}"
        );
    }
}

#[test]
fn type1_rendered_glyph_presence_is_checked_when_charstrings_are_parseable() {
    let missing_cff_bytes = common::minimal_type1c(false);
    let missing_cff = ttf_parser::cff::Table::parse(&missing_cff_bytes).expect("parse missing CFF");
    assert!(missing_cff.glyph_index_by_name("space").is_none());
    let present_cff_bytes = common::minimal_type1c(true);
    let present_cff = ttf_parser::cff::Table::parse(&present_cff_bytes).expect("parse present CFF");
    assert!(present_cff.glyph_index_by_name("space").is_some());
    assert!(
        common::failure_ids(&common::font_fixture("type1_glyph_missing"))
            .contains(TYPE1_GLYPH_PRESENCE)
    );
    assert!(
        !common::failure_ids(&common::font_fixture("type1_glyph_present"))
            .contains(TYPE1_GLYPH_PRESENCE)
    );
    assert!(
        !common::failure_ids(&common::font_fixture("type1_difference_glyph"))
            .contains(TYPE1_GLYPH_PRESENCE)
    );
    let missing_type1c = common::failure_ids(&common::font_fixture("type1c_glyph_missing"));
    assert!(
        missing_type1c.contains(TYPE1_GLYPH_PRESENCE),
        "{missing_type1c:?}"
    );
    assert!(
        !common::failure_ids(&common::font_fixture("type1c_glyph_present"))
            .contains(TYPE1_GLYPH_PRESENCE)
    );
    assert!(
        common::failure_ids(&common::font_fixture("type1c_width_mismatch")).contains(GLYPH_WIDTH)
    );
    assert!(
        common::failure_ids(&common::font_fixture("type1_width_mismatch")).contains(GLYPH_WIDTH)
    );
}

#[test]
fn type1_subset_charset_covers_rendered_embedded_glyphs() {
    assert!(
        common::failure_ids(&common::font_fixture("type1_subset_charset_incomplete"))
            .contains(TYPE1_SUBSET_CHARSET)
    );
    assert!(
        common::failure_ids(&common::font_fixture(
            "type1_subset_charset_difference_incomplete"
        ))
        .contains(TYPE1_SUBSET_CHARSET)
    );
}

#[test]
fn type1c_glyph_presence_matches_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let path = env::temp_dir().join(format!(
        "mai-type1c-missing-glyph-{}.pdf",
        std::process::id()
    ));
    fs::write(&path, common::font_fixture("type1c_glyph_missing")).expect("write CFF fixture");
    let runner = DifferentialRunner::new(ReferenceConfig::pinned(executable)).expect("veraPDF");
    let report = runner.compare_file(&path, &SafetyLimits::default());
    assert_eq!(
        report.classification,
        ComparisonClassification::BothNoncompliant,
        "{report:#?}"
    );
    assert!(
        common::failure_ids(&fs::read(&path).expect("read CFF fixture"))
            .contains(TYPE1_GLYPH_PRESENCE)
    );
    assert!(
        report
            .reference_result
            .expect("veraPDF result")
            .failed_rule_ids
            .iter()
            .map(ToString::to_string)
            .any(|rule| rule == "ISO 19005-1:2005:6.3.5:1")
    );
    fs::remove_file(path).expect("remove CFF fixture");
}

#[test]
fn type1c_width_matches_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let path = env::temp_dir().join(format!("mai-type1c-width-{}.pdf", std::process::id()));
    fs::write(&path, common::font_fixture("type1c_width_mismatch")).expect("write CFF fixture");
    let runner = DifferentialRunner::new(ReferenceConfig::pinned(executable)).expect("veraPDF");
    let report = runner.compare_file(&path, &SafetyLimits::default());
    assert_eq!(
        report.classification,
        ComparisonClassification::BothNoncompliant,
        "{report:#?}"
    );
    assert!(common::failure_ids(&fs::read(&path).expect("read CFF fixture")).contains(GLYPH_WIDTH));
    fs::remove_file(path).expect("remove CFF fixture");
}

#[test]
fn type1_width_matches_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let path = env::temp_dir().join(format!("mai-type1-width-{}.pdf", std::process::id()));
    fs::write(&path, common::font_fixture("type1_width_mismatch")).expect("write Type 1 fixture");
    let runner = DifferentialRunner::new(ReferenceConfig::pinned(executable)).expect("veraPDF");
    let report = runner.compare_file(&path, &SafetyLimits::default());
    assert_eq!(
        report.classification,
        ComparisonClassification::BothNoncompliant,
        "{report:#?}"
    );
    assert!(
        common::failure_ids(&fs::read(&path).expect("read Type 1 fixture")).contains(GLYPH_WIDTH)
    );
    fs::remove_file(path).expect("remove Type 1 fixture");
}

#[test]
fn repeated_aliases_are_one_failure_attached_to_the_font_object() {
    let report = common::validate(&common::font_fixture("repeated_aliases"));
    let failures = font_failures(&report);
    assert_eq!(failures.len(), 1);
    assert!(failures[0].object_id.is_some());
}

#[test]
fn multiple_fonts_are_one_deterministic_unattached_failure() {
    let bytes = common::font_fixture("two_unembedded_fonts");
    let first = common::validate(&bytes);
    let second = common::validate(&bytes);
    let first = font_failures(&first);
    let second = font_failures(&second);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0], second[0]);
    assert!(first[0].object_id.is_none());
    assert_eq!(first[0].message.matches("font object").count(), 2);
}

#[test]
fn serialized_font_summary_shape_is_unchanged() {
    let value = serde_json::to_value(common::validate(&common::font_fixture(
        "unembedded_visible",
    )))
    .expect("serialize report");
    let fonts = value["document"]["fonts"]
        .as_object()
        .expect("serialized font summary");
    assert_eq!(
        fonts.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        BTreeSet::from(["embedded", "total"])
    );
}

#[test]
fn decoded_content_limit_is_an_operational_failure() {
    let limits = SafetyLimits {
        max_decoded_stream_size: 2048,
        ..SafetyLimits::default()
    };
    let bytes = common::font_fixture("large_content");
    PdfDocument::from_bytes(&bytes, &limits)
        .expect("public normalization does not run private font content traversal");
    let report = validate_bytes(&bytes, ValidationProfile::PdfA1b, &limits);
    assert_eq!(report.exit_code(), 1);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].rule_id, "RESOURCE-LIMIT-001");
}

#[test]
fn graphics_state_stack_is_bounded() {
    let limits = SafetyLimits {
        max_reference_depth: 4,
        ..SafetyLimits::default()
    };
    let report = validate_bytes(
        &common::font_fixture("deep_graphics_state"),
        ValidationProfile::PdfA1b,
        &limits,
    );
    assert_eq!(report.exit_code(), 1);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].rule_id, "RESOURCE-LIMIT-001");
}

fn font_failures(
    report: &mai_validation::ValidationReport,
) -> Vec<&mai_validation::ValidationFailure> {
    report
        .failures
        .iter()
        .filter(|failure| failure.rule_id == RULE)
        .collect()
}
