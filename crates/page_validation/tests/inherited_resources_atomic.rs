use std::{env, fs};

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes};

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
fn inherited_resource_names_match_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let directory =
        env::temp_dir().join(format!("page-inherited-resources-{}", std::process::id()));
    fs::create_dir_all(&directory).expect("create inherited-resource fixture directory");
    for (case, expected_failure) in CASES {
        let path = directory.join(format!("{case}.pdf"));
        fs::write(&path, common::graphics_fixture(case)).expect("write fixture");
        for profile in [ValidationProfile::PdfA2b, ValidationProfile::PdfA3b] {
            let report = validate_bytes(
                &common::graphics_fixture(case),
                Some(profile),
                &SafetyLimits::default(),
            )
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
        for (profile, expected_rule) in [
            (ReferenceProfile::PdfA2b, "ISO 19005-2:2011:6.2.2:2"),
            (ReferenceProfile::PdfA3b, "ISO 19005-3:2012:6.2.2:2"),
        ] {
            let mut config = ReferenceConfig::pinned(&executable);
            config.profile = profile;
            let reference = DifferentialRunner::new(config)
                .expect("pinned veraPDF")
                .compare_file(&path, &SafetyLimits::default())
                .reference_result
                .expect("veraPDF result");
            assert_eq!(
                reference
                    .failed_rule_ids
                    .iter()
                    .any(|rule| rule.to_string() == expected_rule),
                *expected_failure,
                "{case}: unexpected {profile} Resources result"
            );
        }
    }
    fs::remove_dir_all(directory).expect("remove inherited-resource fixture directory");
}
