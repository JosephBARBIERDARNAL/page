use std::collections::BTreeSet;
use std::{env, fs};

use page_validation::differential::{
    ComparisonClassification, DifferentialRunner, ReferenceConfig,
};
use page_validation::{PdfDocument, SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFA1B-FONT-EMBEDDING-001";
const TYPE1_GLYPH_PRESENCE: &str = "PDFA1B-TYPE1-GLYPH-PRESENCE-001";
const TYPE1_SUBSET_CHARSET: &str = "PDFA1B-TYPE1-SUBSET-CHARSET-001";
const GLYPH_WIDTH: &str = "PDFA1B-TRUETYPE-GLYPH-WIDTH-001";
const NOTDEF: &str = "PDFA2B-NOTDEF-GLYPH-001";

const CASES: &[(&str, bool)] = &[
    ("unembedded_visible", true),
    ("unembedded_invisible", false),
    ("mixed_rendering_modes", false),
    ("mixed_visible_first", true),
    ("unused_resource", false),
    ("selected_not_shown", false),
    ("direct_font", true),
    ("form_unembedded", true),
    ("form_unembedded_indirect_subtype", true),
    ("nested_form_unembedded", true),
    ("inherited_resources", true),
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
    ("font_subtype_indirect_unembedded", true),
    ("type1c_embedded_indirect_subtype", false),
    ("type1_fontfile_header_only_garbage", true),
    ("type1c_header_only_garbage", true),
];

#[test]
fn font_embedding_cases_have_the_complete_expected_failure_delta() {
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
fn pdfa2_rejects_rendered_notdef_glyphs() {
    let report = validate_bytes_with_profile(
        &common::font_fixture("type3_notdef"),
        ValidationProfile::PdfA2b,
        &SafetyLimits::default(),
    );
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.rule_id == NOTDEF)
    );
}

#[test]
fn pdfa_2_and_3_accept_opentype_subtypes_on_fontfile2() {
    let fixture = common::font_fixture("font_file_subtype_invalid");
    let pdfa1 = validate_bytes_with_profile(
        &fixture,
        ValidationProfile::PdfA1b,
        &SafetyLimits::default(),
    );
    assert!(
        pdfa1
            .failures
            .iter()
            .any(|failure| failure.rule_id == "PDFA1B-FONT-FILE-SUBTYPE-001")
    );

    for profile in [ValidationProfile::PdfA2b, ValidationProfile::PdfA3b] {
        let report = validate_bytes_with_profile(&fixture, profile, &SafetyLimits::default());
        assert!(
            report
                .failures
                .iter()
                .all(|failure| failure.rule_id != "PDFA1B-FONT-FILE-SUBTYPE-001"),
            "{profile}: {report}"
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
    // Confirmed live against veraPDF 1.30.2: a Differences array's code
    // entry as an indirect reference is resolved exactly like a direct
    // value, so the glyph it maps to is still found.
    assert!(
        !common::failure_ids(&common::font_fixture("type1_indirect_difference_code"))
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
fn type1c_default_iso_adobe_charset_finds_rendered_space_glyph() {
    let failures = common::failure_ids(&common::font_fixture("type1c_default_charset_space"));
    assert!(!failures.contains(TYPE1_GLYPH_PRESENCE), "{failures:?}");
}

#[test]
fn type3_rendered_glyphs_must_have_charprocs() {
    let failures = common::failure_ids(&common::font_fixture("type3_visible"));
    assert!(failures.contains(TYPE1_GLYPH_PRESENCE));
    assert!(failures.contains(GLYPH_WIDTH));
}

#[test]
fn type3_charproc_widths_and_predefined_base_encodings_are_checked() {
    for case in [
        "type3_width_match",
        "type3_width_tolerance_boundary",
        "type3_macroman_base",
        "type3_macexpert_base",
    ] {
        let fixture = common::font_fixture(case);
        let report = common::validate(&fixture);
        let failures = common::failure_ids(&fixture);
        assert!(
            !failures.contains(TYPE1_GLYPH_PRESENCE),
            "{case}: {failures:?}"
        );
        assert!(
            !failures.contains(GLYPH_WIDTH),
            "{case}: {failures:?}; {report:#?}"
        );
    }
    for case in ["type3_width_mismatch", "type3_width_d1_mismatch"] {
        let failures = common::failure_ids(&common::font_fixture(case));
        assert!(
            !failures.contains(TYPE1_GLYPH_PRESENCE),
            "{case}: {failures:?}"
        );
        assert!(failures.contains(GLYPH_WIDTH), "{case}: {failures:?}");
    }
    let failures = common::failure_ids(&common::font_fixture("type3_missing_charproc_zero_width"));
    assert!(failures.contains(TYPE1_GLYPH_PRESENCE), "{failures:?}");
    assert!(!failures.contains(GLYPH_WIDTH), "{failures:?}");
}

#[test]
fn type3_width_and_base_encoding_match_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let runner = DifferentialRunner::new(ReferenceConfig::pinned(executable)).expect("veraPDF");
    for (case, should_fail_width) in [
        ("type3_width_match", false),
        ("type3_width_mismatch", true),
        ("type3_width_d1_mismatch", true),
        ("type3_width_tolerance_boundary", false),
        ("type3_missing_charproc_zero_width", false),
        ("type3_macroman_base", false),
        ("type3_macexpert_base", false),
    ] {
        let path = env::temp_dir().join(format!("page-{case}-{}.pdf", std::process::id()));
        fs::write(&path, common::font_fixture(case)).expect("write Type3 fixture");
        let report = runner.compare_file(&path, &SafetyLimits::default());
        let reference_rules = report
            .reference_result
            .as_ref()
            .expect("veraPDF result")
            .failed_rule_ids
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            reference_rules.contains("ISO 19005-1:2005:6.3.6:1"),
            should_fail_width,
            "{case}: {report:#?}"
        );
        assert_eq!(
            common::failure_ids(&fs::read(&path).expect("read Type3 fixture"))
                .contains(GLYPH_WIDTH),
            should_fail_width,
            "{case}: local mismatch"
        );
        fs::remove_file(path).expect("remove Type3 fixture");
    }
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
    // /BaseFont as an indirect reference to a subset-tagged name must still
    // be recognized as a subset font, not silently treated as non-subset.
    assert!(
        common::failure_ids(&common::font_fixture(
            "type1_subset_charset_incomplete_indirect_basefont"
        ))
        .contains(TYPE1_SUBSET_CHARSET)
    );
}

#[test]
fn real_type1_program_encoding_glyphs_charsets_and_widths_are_checked() {
    for case in [
        "type1_real_symbol_present",
        "type1_real_symbol_difference_present",
        "type1_real_symbol_subset_complete",
        "type1_real_symbol_subset_program_encoding_ignored",
    ] {
        let failures = common::failure_ids(&common::font_fixture(case));
        for rule in [TYPE1_GLYPH_PRESENCE, TYPE1_SUBSET_CHARSET, GLYPH_WIDTH] {
            assert!(!failures.contains(rule), "{case}: {failures:?}");
        }
    }

    let failures = common::failure_ids(&common::font_fixture(
        "type1_real_symbol_pdf_base_missing_glyph",
    ));
    assert!(failures.contains(TYPE1_GLYPH_PRESENCE), "{failures:?}");
    assert!(failures.contains(GLYPH_WIDTH), "{failures:?}");

    let failures = common::failure_ids(&common::font_fixture("type1_real_symbol_width_mismatch"));
    assert!(failures.contains(GLYPH_WIDTH), "{failures:?}");

    let failures =
        common::failure_ids(&common::font_fixture("type1_real_symbol_subset_incomplete"));
    assert!(failures.contains(TYPE1_SUBSET_CHARSET), "{failures:?}");
}

#[test]
fn real_type1_program_cases_match_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let runner = DifferentialRunner::new(ReferenceConfig::pinned(executable)).expect("veraPDF");
    for (case, expected_rules) in [
        ("type1_real_symbol_present", &[][..]),
        ("type1_real_symbol_difference_present", &[][..]),
        ("type1_real_symbol_subset_complete", &[][..]),
        ("type1_real_symbol_subset_program_encoding_ignored", &[][..]),
        (
            "type1_real_symbol_pdf_base_missing_glyph",
            &["ISO 19005-1:2005:6.3.5:1", "ISO 19005-1:2005:6.3.6:1"][..],
        ),
        (
            "type1_real_symbol_width_mismatch",
            &["ISO 19005-1:2005:6.3.6:1"][..],
        ),
        (
            "type1_real_symbol_subset_incomplete",
            &["ISO 19005-1:2005:6.3.5:2"][..],
        ),
    ] {
        let path = env::temp_dir().join(format!("page-{case}-{}.pdf", std::process::id()));
        fs::write(&path, common::font_fixture(case)).expect("write real Type1 fixture");
        let report = runner.compare_file(&path, &SafetyLimits::default());
        let reference_rules = report
            .reference_result
            .as_ref()
            .expect("veraPDF result")
            .failed_rule_ids
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        for rule in [
            "ISO 19005-1:2005:6.3.5:1",
            "ISO 19005-1:2005:6.3.5:2",
            "ISO 19005-1:2005:6.3.6:1",
        ] {
            assert_eq!(
                reference_rules.contains(rule),
                expected_rules.contains(&rule),
                "{case}: {report:#?}"
            );
        }
        fs::remove_file(path).expect("remove real Type1 fixture");
    }
}

#[test]
fn type1c_subset_charset_uses_pdf_encoding_names() {
    for case in [
        "type1c_subset_complete",
        "type1c_subset_program_encoding_ignored",
    ] {
        let failures = common::failure_ids(&common::font_fixture(case));
        assert!(
            !failures.contains(TYPE1_SUBSET_CHARSET),
            "{case}: {failures:?}"
        );
    }
    let failures = common::failure_ids(&common::font_fixture("type1c_subset_incomplete"));
    assert!(failures.contains(TYPE1_SUBSET_CHARSET), "{failures:?}");
}

#[test]
fn type1c_subset_charset_cases_match_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let runner = DifferentialRunner::new(ReferenceConfig::pinned(executable)).expect("veraPDF");
    for (case, should_fail) in [
        ("type1c_subset_complete", false),
        ("type1c_subset_incomplete", true),
        ("type1c_subset_program_encoding_ignored", false),
    ] {
        let path = env::temp_dir().join(format!("page-{case}-{}.pdf", std::process::id()));
        fs::write(&path, common::font_fixture(case)).expect("write Type1C fixture");
        let report = runner.compare_file(&path, &SafetyLimits::default());
        let reference_rules = report
            .reference_result
            .as_ref()
            .expect("veraPDF result")
            .failed_rule_ids
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            reference_rules.contains("ISO 19005-1:2005:6.3.5:2"),
            should_fail,
            "{case}: {report:#?}"
        );
        assert_eq!(
            common::failure_ids(&fs::read(&path).expect("read Type1C fixture"))
                .contains(TYPE1_SUBSET_CHARSET),
            should_fail,
            "{case}: local mismatch"
        );
        fs::remove_file(path).expect("remove Type1C fixture");
    }
}

#[test]
fn external_type1_programs_can_be_screened_against_pinned_verapdf() {
    let (Some(executable), Some(programs)) = (
        env::var_os("VERAPDF_BIN"),
        env::var_os("PAGE_TYPE1_PROGRAMS"),
    ) else {
        return;
    };
    let runner = DifferentialRunner::new(ReferenceConfig::pinned(executable)).expect("veraPDF");
    for program_path in env::split_paths(&programs) {
        let program = fs::read(&program_path).expect("read external Type1 program");
        let fixture =
            common::font_fixture_with_external_type1_program("type1_real_symbol_present", &program);
        let path = env::temp_dir().join(format!(
            "page-type1-screen-{}-{}.pdf",
            std::process::id(),
            program_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
        ));
        fs::write(&path, fixture).expect("write screened Type1 fixture");
        let report = runner.compare_file(&path, &SafetyLimits::default());
        eprintln!(
            "{}: {:?} {:?}",
            program_path.display(),
            report.classification,
            report
                .reference_result
                .as_ref()
                .map(|result| &result.failed_rule_ids)
        );
        fs::remove_file(path).expect("remove screened Type1 fixture");
    }
}

#[test]
fn type1c_glyph_presence_matches_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let path = env::temp_dir().join(format!(
        "page-type1c-missing-glyph-{}.pdf",
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
    let path = env::temp_dir().join(format!("page-type1c-width-{}.pdf", std::process::id()));
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
    let path = env::temp_dir().join(format!("page-type1-width-{}.pdf", std::process::id()));
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
    let report = validate_bytes_with_profile(&bytes, ValidationProfile::PdfA1b, &limits);
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
    let report = validate_bytes_with_profile(
        &common::font_fixture("deep_graphics_state"),
        ValidationProfile::PdfA1b,
        &limits,
    );
    assert_eq!(report.exit_code(), 1);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].rule_id, "RESOURCE-LIMIT-001");
}

fn font_failures(
    report: &page_validation::ValidationReport,
) -> Vec<&page_validation::ValidationFailure> {
    report
        .failures
        .iter()
        .filter(|failure| failure.rule_id == RULE)
        .collect()
}
