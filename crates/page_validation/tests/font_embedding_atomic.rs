use std::collections::BTreeSet;

use page_validation::{
    PdfDocument, PdfError, SafetyLimits, ValidationError, ValidationProfile, validate_pdf_bytes,
};

pub mod common;

const RULE: &str = "PDFA1B-FONT-EMBEDDING-001";

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
    assert!(first[0].message.contains("font object"));
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
    let error = validate_pdf_bytes(&bytes, Some(ValidationProfile::PdfA1b), &limits)
        .expect_err("decoded content must exceed the configured limit");
    assert!(matches!(
        error,
        ValidationError::Pdf(PdfError::ContentDecodeLimit(2048))
    ));
}

#[test]
fn graphics_state_stack_is_bounded() {
    let limits = SafetyLimits {
        max_reference_depth: 4,
        ..SafetyLimits::default()
    };
    let error = validate_pdf_bytes(
        &common::font_fixture("deep_graphics_state"),
        Some(ValidationProfile::PdfA1b),
        &limits,
    )
    .expect_err("graphics state must exceed the configured reference depth");
    assert!(matches!(
        error,
        ValidationError::Pdf(PdfError::ReferenceDepth(4))
    ));
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
