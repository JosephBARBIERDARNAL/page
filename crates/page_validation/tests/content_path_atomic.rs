pub mod common;

const VIOLATIONS: &[(&str, &str)] = &[
    ("undefined", "PDFA1B-CONTENT-OPERATOR-001"),
    ("extgstate_tr", "PDFA1B-EXTGSTATE-TR-001"),
    ("image_bpc_16", "PDFA1B-IMAGE-BPC-001"),
    ("nesting_29", "PDFA1B-GRAPHICS-STATE-NESTING-001"),
    ("inline_lzw", "PDFA1B-INLINE-IMAGE-LZW-001"),
    ("invalid_intent", "PDFA1B-RENDERING-INTENT-001"),
];

#[test]
fn every_content_source_runs_the_shared_rule_population() {
    let baseline = common::failure_ids(&common::graphics_fixture("baseline"));
    for source in ["form", "appearance", "pattern", "type3"] {
        for (violation, rule) in VIOLATIONS {
            let case = format!("path_{source}_{violation}");
            let actual = common::failure_ids(&common::graphics_fixture(&case));
            let added = actual.difference(&baseline).cloned().collect::<Vec<_>>();
            assert_eq!(added, [*rule], "{case}");
        }
    }
}

#[test]
fn source_resource_fallback_matches_the_shared_executor_contract() {
    let baseline = common::failure_ids(&common::graphics_fixture("baseline"));
    for source in ["form", "appearance", "pattern", "type3"] {
        for variant in [
            "fallback_extgstate_tr",
            "missing_resources_fallback_extgstate_tr",
        ] {
            let case = format!("path_{source}_{variant}");
            let actual = common::failure_ids(&common::graphics_fixture(&case));
            let added = actual.difference(&baseline).cloned().collect::<Vec<_>>();
            assert_eq!(added, ["PDFA1B-EXTGSTATE-TR-001"], "{case}");
        }
    }
}

#[test]
fn nested_form_fallback_uses_the_page_not_the_invoking_form_resources() {
    let baseline = common::failure_ids(&common::graphics_fixture("baseline"));
    for case in [
        "path_form_parent_only_extgstate_tr",
        "path_form_missing_resources_parent_extgstate_tr",
    ] {
        let actual = common::failure_ids(&common::graphics_fixture(case));
        let added = actual.difference(&baseline).cloned().collect::<Vec<_>>();
        assert!(added.is_empty(), "{case}");
    }
}

#[test]
fn appearance_stream_role_does_not_require_an_explicit_form_subtype() {
    let baseline = common::failure_ids(&common::graphics_fixture("baseline"));
    let actual = common::failure_ids(&common::graphics_fixture(
        "path_appearance_missing_subtype_undefined",
    ));
    let added = actual.difference(&baseline).cloned().collect::<Vec<_>>();
    assert_eq!(added, ["PDFA1B-CONTENT-OPERATOR-001"]);

    let actual = common::failure_ids(&common::graphics_fixture(
        "path_appearance_missing_subtype_form_ref",
    ));
    let added = actual.difference(&baseline).cloned().collect::<Vec<_>>();
    assert_eq!(added, ["PDFA1B-FORM-REFERENCE-001"]);

    let actual = common::failure_ids(&common::graphics_fixture(
        "path_appearance_image_subtype_bpc_16",
    ));
    let added = actual.difference(&baseline).cloned().collect::<Vec<_>>();
    assert!(added.is_empty());

    let actual = common::failure_ids(&common::graphics_fixture(
        "path_appearance_appearance_and_painted_image_bpc_16",
    ));
    let added = actual.difference(&baseline).cloned().collect::<Vec<_>>();
    assert_eq!(added, ["PDFA1B-IMAGE-BPC-001"]);
}

#[test]
fn malformed_nested_appearance_states_follow_the_pinned_recovery_model() {
    let baseline = common::failure_ids(&common::graphics_fixture("baseline"));
    let actual = common::failure_ids(&common::graphics_fixture(
        "path_appearance_nested_state_undefined",
    ));
    let added = actual.difference(&baseline).cloned().collect::<Vec<_>>();
    assert_eq!(added, ["PDFA1B-ANNOTATION-NORMAL-APPEARANCE-001"]);
}

#[test]
fn pattern_resource_presence_controls_the_invoking_resource_fallback() {
    let baseline = common::failure_ids(&common::graphics_fixture("baseline"));
    for case in [
        "path_pattern_missing_resources_parent_extgstate_tr",
        "path_pattern_empty_resources_parent_extgstate_tr",
    ] {
        let actual = common::failure_ids(&common::graphics_fixture(case));
        let added = actual.difference(&baseline).cloned().collect::<Vec<_>>();
        assert!(added.is_empty(), "{case}");
    }
}

#[test]
fn type3_resource_presence_does_not_inherit_the_invoking_form_resources() {
    let baseline = common::failure_ids(&common::graphics_fixture("baseline"));
    for case in [
        "path_type3_missing_resources_parent_extgstate_tr",
        "path_type3_empty_resources_parent_extgstate_tr",
    ] {
        let actual = common::failure_ids(&common::graphics_fixture(case));
        let added = actual.difference(&baseline).cloned().collect::<Vec<_>>();
        assert!(added.is_empty(), "{case}");
    }
}

#[test]
fn invisible_type3_glyph_content_is_still_executed() {
    let baseline = common::failure_ids(&common::graphics_fixture("baseline"));
    let actual = common::failure_ids(&common::graphics_fixture("path_type3_invisible_undefined"));
    let added = actual.difference(&baseline).cloned().collect::<Vec<_>>();
    assert_eq!(added, ["PDFA1B-CONTENT-OPERATOR-001"]);
}

#[test]
fn malformed_text_show_still_reaches_the_type3_charproc() {
    let baseline = common::failure_ids(&common::graphics_fixture("baseline"));
    let actual = common::failure_ids(&common::graphics_fixture(
        "path_type3_malformed_text_show_undefined",
    ));
    let added = actual.difference(&baseline).cloned().collect::<Vec<_>>();
    assert_eq!(added, ["PDFA1B-CONTENT-OPERATOR-001"]);
}

#[test]
fn type3_charproc_inherits_the_callers_pattern_selection() {
    let baseline = common::failure_ids(&common::graphics_fixture("baseline"));
    let actual = common::failure_ids(&common::graphics_fixture(
        "path_type3_inherited_pattern_undefined",
    ));
    let added = actual.difference(&baseline).cloned().collect::<Vec<_>>();
    assert_eq!(added, ["PDFA1B-CONTENT-OPERATOR-001"]);
}

#[test]
fn soft_mask_group_contents_use_the_smasks_restriction() {
    let baseline = common::failure_ids(&common::graphics_fixture("baseline"));
    for (violation, _) in VIOLATIONS {
        let case = format!("path_soft_mask_{violation}");
        let actual = common::failure_ids(&common::graphics_fixture(&case));
        let added = actual.difference(&baseline).cloned().collect::<Vec<_>>();
        assert_eq!(added, ["PDFA1B-EXTGSTATE-SMASK-001"], "{case}");
    }
}
