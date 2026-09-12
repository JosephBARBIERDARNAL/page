pub mod common;

use lopdf::{Object, dictionary};
use page_validation::{
    PdfError, SafetyLimits, ValidationError, ValidationProfile, validate_pdf_bytes,
};

#[test]
fn role_map_cycles_are_rejected_but_acyclic_chains_are_accepted() {
    for (case, should_fail) in [
        ("struct_tree_role_map_self_cycle", true),
        ("struct_tree_role_map_two_node_cycle", true),
        ("struct_tree_role_map_long_cycle", true),
        ("struct_tree_role_map_acyclic_chain", false),
    ] {
        let report = validate_pdf_bytes(
            &common::tagged_document_fixture(case),
            Some(ValidationProfile::PdfA1a),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert_eq!(
            report
                .failures
                .iter()
                .any(|failure| failure.rule_id == "PDFA1A-STRUCT-TREE-ROLE-MAP-CYCLE-001"),
            should_fail,
            "{case}: {report:#?}"
        );
    }
}

#[test]
fn role_map_traversal_limit_does_not_create_a_conformance_failure() {
    let limits = SafetyLimits {
        max_object_count: 1,
        ..SafetyLimits::default()
    };
    let error = validate_pdf_bytes(
        &common::tagged_document_fixture("struct_tree_role_map_self_cycle"),
        Some(ValidationProfile::PdfA1a),
        &limits,
    )
    .expect_err("the object limit must stop the traversal");
    assert!(matches!(
        error,
        ValidationError::Pdf(PdfError::TooManyObjects { limit: 1, .. })
    ));
}

#[test]
fn cyclic_structure_tree_is_an_operational_failure() {
    let error = validate_pdf_bytes(
        &common::tagged_document_fixture("struct_tree_cyclic"),
        Some(ValidationProfile::PdfA1a),
        &SafetyLimits::default(),
    )
    .expect_err("cyclic structure tree must exceed the reference-depth limit");
    assert!(matches!(error, ValidationError::Pdf(_)));
}

#[test]
fn table_grid_row_limit_is_rejected_during_structure_inspection() {
    let mut document = common::pdf_document();
    let struct_tree_root_id = document.new_object_id();
    let table_id = document.new_object_id();
    let table_body_id = document.new_object_id();
    let row_ids = (0..2).map(|_| document.new_object_id()).collect::<Vec<_>>();

    for row_id in row_ids.iter().copied() {
        document.objects.insert(
            row_id,
            Object::Dictionary(dictionary! {
                "S" => "TR",
                "P" => Object::Reference(table_body_id),
            }),
        );
    }
    document.objects.insert(
        table_body_id,
        Object::Dictionary(dictionary! {
            "S" => "TBody",
            "P" => Object::Reference(table_id),
            "K" => row_ids
                .iter()
                .copied()
                .map(Object::Reference)
                .collect::<Vec<_>>(),
        }),
    );
    document.objects.insert(
        table_id,
        Object::Dictionary(dictionary! {
            "S" => "Table",
            "P" => Object::Reference(struct_tree_root_id),
            "K" => Object::Reference(table_body_id),
        }),
    );
    document.objects.insert(
        struct_tree_root_id,
        Object::Dictionary(dictionary! {
            "K" => Object::Reference(table_id),
        }),
    );
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "StructTreeRoot" => Object::Reference(struct_tree_root_id),
    });
    document.trailer.set("Root", Object::Reference(catalog_id));

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save table row-limit fixture");

    let limits = SafetyLimits {
        max_table_grid_rows: 1,
        ..SafetyLimits::default()
    };
    let error = validate_pdf_bytes(&bytes, Some(ValidationProfile::PdfA1a), &limits)
        .expect_err("table inspection must reject rows before growing past the limit");
    assert!(matches!(
        error,
        ValidationError::Pdf(PdfError::TableGridLimit {
            rows: 2,
            max_rows: 1,
            ..
        })
    ));
}
