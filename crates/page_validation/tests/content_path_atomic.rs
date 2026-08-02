use std::collections::BTreeMap;
use std::fs;

#[allow(dead_code)]
mod common;

const VIOLATIONS: &[(&str, &str, &str)] = &[
    (
        "undefined",
        "PDFA1B-CONTENT-OPERATOR-001",
        "ISO 19005-1:2005:6.2.10:1",
    ),
    (
        "extgstate_tr",
        "PDFA1B-EXTGSTATE-TR-001",
        "ISO 19005-1:2005:6.2.8:1",
    ),
    (
        "image_bpc_16",
        "PDFA1B-IMAGE-BPC-001",
        "ISO 19005-1:2005:6.2.4:4",
    ),
    (
        "nesting_29",
        "PDFA1B-GRAPHICS-STATE-NESTING-001",
        "ISO 19005-1:2005:6.1.12:8",
    ),
    (
        "inline_lzw",
        "PDFA1B-INLINE-IMAGE-LZW-001",
        "ISO 19005-1:2005:6.1.10:2",
    ),
    (
        "invalid_intent",
        "PDFA1B-RENDERING-INTENT-001",
        "ISO 19005-1:2005:6.2.9:1",
    ),
];

#[test]
fn every_verapdf_content_source_runs_the_shared_rule_population() {
    let baseline = common::failure_ids(&common::graphics_fixture("baseline"));
    for source in ["form", "appearance", "pattern", "type3"] {
        for (violation, rule, _) in VIOLATIONS {
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
fn verapdf_1_28_2_does_not_execute_soft_mask_group_contents() {
    let baseline = common::failure_ids(&common::graphics_fixture("baseline"));
    for (violation, _, _) in VIOLATIONS {
        let case = format!("path_soft_mask_{violation}");
        let actual = common::failure_ids(&common::graphics_fixture(&case));
        let added = actual.difference(&baseline).cloned().collect::<Vec<_>>();
        assert_eq!(added, ["PDFA1B-EXTGSTATE-SMASK-001"], "{case}");
    }
}

#[test]
fn differential_manifest_pins_the_complete_content_path_matrix() {
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read("tests/fixtures/verapdf-diff-cases.json").expect("read differential manifest"),
    )
    .expect("parse differential manifest");
    let cases = manifest["atomic_graphics_cases"]
        .as_array()
        .expect("atomic graphics cases")
        .iter()
        .map(|case| (case["name"].as_str().expect("case name"), case))
        .collect::<BTreeMap<_, _>>();

    for source in ["form", "appearance", "pattern", "type3"] {
        for (violation, local_rule, verapdf_rule) in VIOLATIONS {
            let name = format!("path_{source}_{violation}");
            let case = cases
                .get(name.as_str())
                .unwrap_or_else(|| panic!("missing differential case {name}"));
            assert_rule_membership(case, "expected_local_failed_rule_ids", local_rule, &name);
            assert_rule_membership(
                case,
                "expected_verapdf_failed_rule_ids",
                verapdf_rule,
                &name,
            );
        }
        let name = format!("path_{source}_fallback_extgstate_tr");
        let case = cases
            .get(name.as_str())
            .unwrap_or_else(|| panic!("missing differential case {name}"));
        assert_rule_membership(
            case,
            "expected_local_failed_rule_ids",
            "PDFA1B-EXTGSTATE-TR-001",
            &name,
        );
        assert_rule_membership(
            case,
            "expected_verapdf_failed_rule_ids",
            "ISO 19005-1:2005:6.2.8:1",
            &name,
        );
        let name = format!("path_{source}_missing_resources_fallback_extgstate_tr");
        let case = cases
            .get(name.as_str())
            .unwrap_or_else(|| panic!("missing differential case {name}"));
        assert_rule_membership(
            case,
            "expected_local_failed_rule_ids",
            "PDFA1B-EXTGSTATE-TR-001",
            &name,
        );
        assert_rule_membership(
            case,
            "expected_verapdf_failed_rule_ids",
            "ISO 19005-1:2005:6.2.8:1",
            &name,
        );
    }

    for (violation, local_rule, verapdf_rule) in VIOLATIONS {
        let name = format!("path_soft_mask_{violation}");
        let case = cases
            .get(name.as_str())
            .unwrap_or_else(|| panic!("missing differential case {name}"));
        assert_rule_membership(
            case,
            "expected_local_failed_rule_ids",
            "PDFA1B-EXTGSTATE-SMASK-001",
            &name,
        );
        assert_rule_membership(
            case,
            "expected_verapdf_failed_rule_ids",
            "ISO 19005-1:2005:6.4:1",
            &name,
        );
        assert_rule_membership(case, "expected_local_passed_rule_ids", local_rule, &name);
        assert_rule_membership(
            case,
            "expected_verapdf_passed_rule_ids",
            verapdf_rule,
            &name,
        );
    }

    for name in [
        "path_form_parent_only_extgstate_tr",
        "path_form_missing_resources_parent_extgstate_tr",
        "path_appearance_missing_subtype_undefined",
        "path_appearance_missing_subtype_form_ref",
        "path_appearance_image_subtype_bpc_16",
        "path_appearance_appearance_and_painted_image_bpc_16",
        "path_appearance_nested_state_undefined",
        "path_pattern_missing_resources_parent_extgstate_tr",
        "path_pattern_empty_resources_parent_extgstate_tr",
        "path_type3_missing_resources_parent_extgstate_tr",
        "path_type3_empty_resources_parent_extgstate_tr",
        "unused_form_undefined",
        "unused_appearance_undefined",
        "unused_pattern_undefined",
        "unused_type3_undefined",
        "unused_soft_mask_group_undefined",
    ] {
        assert!(cases.contains_key(name), "missing differential case {name}");
    }
}

fn assert_rule_membership(case: &serde_json::Value, field: &str, rule: &str, case_name: &str) {
    assert!(
        case[field]
            .as_array()
            .expect("rule ID array")
            .iter()
            .any(|value| value.as_str() == Some(rule)),
        "{case_name} does not pin {rule} in {field}"
    );
}
