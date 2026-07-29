use std::collections::BTreeSet;

use mai_validation::{PdfDocument, SafetyLimits, ValidationProfile, validate_bytes};

#[allow(dead_code)]
mod common;

const RULE: &str = "PDFA1B-FONT-EMBEDDING-001";

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
