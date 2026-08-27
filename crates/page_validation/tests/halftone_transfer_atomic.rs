use std::{env, fs};

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_pdf_bytes};

pub mod common;

const CASES: &[(&str, bool)] = &[
    ("halftone_transfer_root_invalid", true),
    ("halftone_transfer_root_indirect_ht_invalid", true),
    ("halftone_transfer_unreferenced_invalid", false),
    ("halftone_transfer_unused_invalid", false),
    ("halftone_transfer_root_null", false),
    ("halftone_transfer_root_indirect_null", false),
    ("halftone_transfer_primary_invalid", true),
    ("halftone_transfer_spot_missing", true),
    ("halftone_transfer_default_present", false),
    ("halftone_transfer_spot_present", false),
];

#[test]
fn halftone_transfer_function_cases_match_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let directory = env::temp_dir().join(format!("page-halftone-transfer-{}", std::process::id()));
    fs::create_dir_all(&directory).expect("create halftone fixture directory");
    for (case, expected_failure) in CASES {
        let path = directory.join(format!("{case}.pdf"));
        let bytes = common::graphics_fixture(case);
        fs::write(&path, &bytes).expect("write halftone fixture");
        for profile in [ValidationProfile::PdfA2b, ValidationProfile::PdfA3b] {
            assert_eq!(
                validate_pdf_bytes(&bytes, Some(profile), &SafetyLimits::default())
                    .expect("explicit profile validation")
                    .failures
                    .iter()
                    .any(|failure| {
                        failure.rule_id == "PDFA2B-HALFTONE-TRANSFER-FUNCTION-001"
                            || failure.rule_id == "PDFA3B-HALFTONE-TRANSFER-FUNCTION-001"
                    }),
                *expected_failure,
                "{case}: unexpected local {profile} TransferFunction result"
            );
        }
        for (profile, expected_rule) in [
            (ReferenceProfile::PdfA2b, "ISO 19005-2:2011:6.2.5:6"),
            (ReferenceProfile::PdfA3b, "ISO 19005-3:2012:6.2.5:6"),
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
                "{case}: unexpected {profile} TransferFunction result"
            );
        }
    }
    fs::remove_dir_all(directory).expect("remove halftone fixture directory");
}

#[test]
fn selected_halftones_match_pinned_verapdf_across_content_paths_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let directory = env::temp_dir().join(format!("page-halftone-paths-{}", std::process::id()));
    fs::create_dir_all(&directory).expect("create halftone path fixture directory");
    for source in ["form", "appearance", "pattern", "type3"] {
        let case = format!("path_{source}_halftone_transfer_invalid");
        let bytes = common::graphics_fixture(&case);
        let path = directory.join(format!("{source}.pdf"));
        fs::write(&path, &bytes).expect("write halftone path fixture");
        for profile in [ValidationProfile::PdfA2b, ValidationProfile::PdfA3b] {
            assert!(
                validate_pdf_bytes(&bytes, Some(profile), &SafetyLimits::default())
                    .expect("explicit profile validation")
                    .failures
                    .iter()
                    .any(|failure| failure.rule_id.ends_with("HALFTONE-TRANSFER-FUNCTION-001")),
                "{case}: local {profile} did not report the selected halftone"
            );
        }
        for (profile, rule) in [
            (ReferenceProfile::PdfA2b, "ISO 19005-2:2011:6.2.5:6"),
            (ReferenceProfile::PdfA3b, "ISO 19005-3:2012:6.2.5:6"),
        ] {
            let mut config = ReferenceConfig::pinned(&executable);
            config.profile = profile;
            assert!(
                DifferentialRunner::new(config)
                    .expect("pinned veraPDF")
                    .compare_file(&path, &SafetyLimits::default())
                    .reference_result
                    .expect("veraPDF result")
                    .failed_rule_ids
                    .iter()
                    .any(|failed| failed.to_string() == rule),
                "{case}: veraPDF {profile} did not report the selected halftone"
            );
        }
    }
    fs::remove_dir_all(directory).expect("remove halftone path fixture directory");
}
