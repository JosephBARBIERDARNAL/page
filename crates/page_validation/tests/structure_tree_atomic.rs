pub mod common;

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
