use std::collections::BTreeSet;

use lopdf::{Document, Object, dictionary};
use page_validation::{
    PageError, PdfError, SafetyLimits, ValidationOptions, ValidationProfile,
    is_pdf_compliant_bytes, validate_pdf_bytes,
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
fn no_shown_text_skips_font_details_but_keeps_the_informational_summary() {
    let report = common::validate(&common::font_fixture("unused_resource"));

    assert!(font_failures(&report).is_empty());
    let value = serde_json::to_value(&report).expect("serialize report");
    assert_eq!(value["document"]["fonts"]["total"], 1);
}

#[test]
fn lazy_validation_skips_unused_font_summary_resolution() {
    let mut document = Document::load_mem(&common::font_fixture("unused_resource"))
        .expect("load unused-font fixture");
    let limits = SafetyLimits::default();
    let terminal = document.add_object(dictionary! {});
    let mut descriptor = Object::Reference(terminal);
    for _ in 0..=limits.max_reference_depth() {
        descriptor = Object::Reference(document.add_object(descriptor));
    }
    document.add_object(dictionary! {
        "Type" => "Font",
        "FontDescriptor" => descriptor,
    });
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save unused-font fixture");

    let options = ValidationOptions::default()
        .profile(ValidationProfile::PdfA1b)
        .limits(limits);
    assert!(is_pdf_compliant_bytes(&bytes, &options).unwrap());
    assert!(matches!(
        validate_pdf_bytes(&bytes, &options),
        Err(PageError::Pdf(PdfError::ReferenceDepth(
            SafetyLimits::DEFAULT_MAX_REFERENCE_DEPTH
        )))
    ));
}

#[test]
fn decoded_content_limit_is_an_operational_failure() {
    let limits = SafetyLimits::default().with_max_decoded_stream_size(2048);
    let bytes = common::font_fixture("large_content");
    let error = validate_pdf_bytes(
        &bytes,
        &ValidationOptions::default()
            .profile(ValidationProfile::PdfA1b)
            .limits(limits),
    )
    .expect_err("decoded content must exceed the configured limit");
    assert!(matches!(
        error,
        PageError::Pdf(PdfError::ContentDecodeLimit(2048))
    ));
}

#[test]
fn graphics_state_stack_is_bounded() {
    let limits = SafetyLimits::default().with_max_reference_depth(4);
    let error = validate_pdf_bytes(
        &common::font_fixture("deep_graphics_state"),
        &ValidationOptions::default()
            .profile(ValidationProfile::PdfA1b)
            .limits(limits),
    )
    .expect_err("graphics state must exceed the configured reference depth");
    assert!(matches!(error, PageError::Pdf(PdfError::ReferenceDepth(4))));
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
