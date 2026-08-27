use std::{env, fs};

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_pdf_bytes};

pub mod common;

#[test]
fn separation_consistency_matches_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let directory = env::temp_dir().join(format!(
        "page-separation-consistency-{}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).expect("create fixture directory");
    for (case, expected) in [
        ("separation_consistent", false),
        ("separation_inconsistent", true),
        ("separation_unreferenced_inconsistent", false),
    ] {
        let bytes = common::color_path_fixture(case);
        let path = directory.join(format!("{case}.pdf"));
        fs::write(&path, &bytes).expect("write fixture");
        for profile in [ValidationProfile::PdfA2b, ValidationProfile::PdfA3b] {
            assert_eq!(
                validate_pdf_bytes(&bytes, Some(profile), &SafetyLimits::default())
                    .expect("explicit profile validation")
                    .failures
                    .iter()
                    .any(|f| f.rule_id.ends_with("SEPARATION-CONSISTENCY-001")),
                expected,
                "{case}: local {profile}"
            );
        }
        for (profile, rule) in [
            (ReferenceProfile::PdfA2b, "ISO 19005-2:2011:6.2.4.4:2"),
            (ReferenceProfile::PdfA3b, "ISO 19005-3:2012:6.2.4.4:2"),
        ] {
            let mut config = ReferenceConfig::pinned(&executable);
            config.profile = profile;
            let result = DifferentialRunner::new(config)
                .expect("veraPDF")
                .compare_file(&path, &SafetyLimits::default())
                .reference_result
                .expect("reference result");
            assert_eq!(
                result.failed_rule_ids.iter().any(|f| f.to_string() == rule),
                expected,
                "{case}: veraPDF {profile}"
            );
        }
    }
    fs::remove_dir_all(directory).expect("remove fixture directory");
}
