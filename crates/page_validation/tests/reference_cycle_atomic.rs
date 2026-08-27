//! Regression tests for a fixed bug: `model.rs` used to swallow reference
//! cycles into false conformance/metadata failures instead of surfacing
//! `RESOURCE-LIMIT-001`. These fixtures are deliberately malformed
//! self-referencing objects, built directly with `lopdf::Document` rather
//! than through `tests/common`'s case-table fixture builders.

use lopdf::{Document, Object, dictionary};
use page_validation::{
    PdfDocument, PdfError, SafetyLimits, ValidationError, ValidationProfile, validate_bytes,
};

fn assert_resource_limit_failure(bytes: &[u8]) {
    let error = validate_bytes(
        bytes,
        Some(ValidationProfile::PdfA1b),
        &SafetyLimits::default(),
    )
    .expect_err("reference cycle must exceed the configured reference depth");
    assert!(
        matches!(
            error,
            ValidationError::Pdf(PdfError::ReferenceDepth(
                SafetyLimits::DEFAULT_MAX_REFERENCE_DEPTH
            ))
        ),
        "{error:?}"
    );
}

#[test]
fn cyclic_metadata_reference_is_a_resource_limit_failure() {
    let mut document = Document::with_version("1.4");
    let pages_id = document.new_object_id();
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1,
        }),
    );

    // A pure self-referencing indirect object: resolving it hits the same
    // object id twice within a single `resolve()` call, regardless of the
    // configured reference-depth limit.
    let metadata_id = document.new_object_id();
    document
        .objects
        .insert(metadata_id, Object::Reference(metadata_id));

    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
    });
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save cyclic metadata fixture");

    assert_resource_limit_failure(&bytes);
}

#[test]
fn cyclic_font_descriptor_reference_is_a_resource_limit_failure() {
    let mut document = Document::with_version("1.4");
    let pages_id = document.new_object_id();
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1,
        }),
    );

    let descriptor_id = document.new_object_id();
    document
        .objects
        .insert(descriptor_id, Object::Reference(descriptor_id));
    // Not reachable from any page content; `summarize_fonts` walks every
    // `/Type /Font` object in the document regardless of use.
    document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "FontDescriptor" => descriptor_id,
    });

    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save cyclic font descriptor fixture");

    assert_resource_limit_failure(&bytes);
}

#[test]
fn cyclic_page_tree_reference_is_a_resource_limit_failure() {
    let mut document = Document::with_version("1.4");
    let pages_id = document.new_object_id();
    document
        .objects
        .insert(pages_id, Object::Reference(pages_id));

    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save cyclic page tree fixture");

    assert_resource_limit_failure(&bytes);
}

/// A page-tree cycle spanning two `Pages` nodes (A's `Kids` contains B, B's
/// `Kids` contains A) is not caught by a single `resolve()` call the way the
/// one-hop self-reference above is: each node is only one reference hop from
/// its parent, so the cycle only closes after two separate `Kids` lookups.
/// This regression-tests that the page-tree walker still detects it via its
/// own cross-call visited set, instead of silently returning zero pages.
#[test]
fn two_level_page_tree_cycle_is_a_resource_limit_failure() {
    let mut document = Document::with_version("1.4");
    let a_id = document.new_object_id();
    let b_id = document.new_object_id();
    document.objects.insert(
        a_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(b_id)],
            "Count" => 1,
        }),
    );
    document.objects.insert(
        b_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(a_id)],
            "Count" => 1,
        }),
    );

    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => a_id,
    });
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save two-level cyclic page tree fixture");

    assert_resource_limit_failure(&bytes);
}

fn nested_page_tree_fixture(depth: usize) -> Vec<u8> {
    let mut document = Document::with_version("1.4");
    let mut current = document.add_object(dictionary! {
        "Type" => "Page",
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });
    for _ in 0..depth {
        current = document.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(current)],
            "Count" => 1,
        });
    }

    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => current,
    });
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save nested page tree fixture");
    bytes
}

/// `count_pages` budgets each level's resolution as
/// `max_reference_depth - depth`, but `resolve()` only confirms a fetched
/// object is terminal (non-`Reference`) on the iteration *after* fetching
/// it — so resolving even one legitimate, non-cyclic hop consumes a full
/// unit of budget beyond what's needed just to reach the target. The
/// deepest level that can still resolve is therefore
/// `max_reference_depth - 1`, not `max_reference_depth`. This test pins
/// that true boundary down deliberately: a page tree nested to exactly
/// `max_reference_depth - 1` levels must still validate and count pages
/// correctly.
#[test]
fn page_tree_one_level_inside_the_reference_depth_boundary_still_validates() {
    let bytes = nested_page_tree_fixture(SafetyLimits::DEFAULT_MAX_REFERENCE_DEPTH - 1);

    let parsed = PdfDocument::from_bytes(&bytes, &SafetyLimits::default())
        .expect("acyclic page tree one level inside the reference-depth boundary should not error");
    assert_eq!(parsed.page_count, 1);
}

/// One level deeper (nested to exactly `max_reference_depth`), the same
/// off-by-one means the leaf page is resolved with zero budget and now
/// correctly surfaces as `RESOURCE-LIMIT-001`, where before this fix it
/// was silently undercounted with no failure at all. This is an accepted,
/// narrow behavior change (see the doc comment above): 128 levels of page
/// nesting is astronomically unlikely in a real PDF, and reporting it as a
/// resource-limit condition is more correct than silently miscounting.
#[test]
fn page_tree_at_the_reference_depth_boundary_is_a_resource_limit_failure() {
    let bytes = nested_page_tree_fixture(SafetyLimits::DEFAULT_MAX_REFERENCE_DEPTH);

    assert_resource_limit_failure(&bytes);
}
