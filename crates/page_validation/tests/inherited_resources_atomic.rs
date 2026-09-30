use page_validation::{ValidationOptions, ValidationProfile, validate_pdf_bytes};

pub mod common;

const CASES: &[(&str, bool)] = &[
    ("extgstate_tr", false),
    ("inherited_extgstate_tr", true),
    ("inherited_resource_color_space", false),
    ("inherited_resource_calgray", true),
    ("inherited_resource_default_color_space", false),
    ("inherited_resource_extgstate", true),
    ("inherited_resource_font", true),
    ("inherited_resource_xobject", true),
    ("inherited_resource_pattern", true),
    ("inherited_resource_shading", true),
    ("inherited_resource_properties", true),
    ("path_form_extgstate_tr", false),
    ("path_form_fallback_extgstate_tr", true),
    ("path_form_missing_resources_fallback_extgstate_tr", true),
    ("path_appearance_extgstate_tr", false),
    ("path_appearance_fallback_extgstate_tr", true),
    (
        "path_appearance_missing_resources_fallback_extgstate_tr",
        true,
    ),
    ("path_pattern_extgstate_tr", false),
    ("path_pattern_fallback_extgstate_tr", true),
    ("path_pattern_missing_resources_fallback_extgstate_tr", true),
    ("path_type3_extgstate_tr", false),
    ("path_type3_fallback_extgstate_tr", true),
    ("path_type3_missing_resources_fallback_extgstate_tr", true),
];

#[test]
fn inherited_resource_names_are_checked_in_pdfa_2_and_3() {
    for (case, expected_failure) in CASES {
        let bytes = common::graphics_fixture(case);
        for profile in [ValidationProfile::PdfA2b, ValidationProfile::PdfA3b] {
            let report = validate_pdf_bytes(&bytes, &ValidationOptions::default().profile(profile))
                .expect("explicit profile validation");
            assert_eq!(
                report
                    .failures
                    .iter()
                    .any(|failure| failure.rule_id.ends_with("CONTENT-RESOURCES-001")),
                *expected_failure,
                "{case}: unexpected local {profile} Resources result"
            );
        }
    }
}
