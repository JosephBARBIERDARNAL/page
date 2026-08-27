use std::{env, fs};

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_pdf_bytes};

pub mod common;

const CASES: &[(&str, bool)] = &[
    ("stroke_opm_one", true),
    ("fill_opm_one", true),
    ("stroke_opm_zero", false),
    ("stroke_overprint_false", false),
    ("select_before_state", true),
];

#[test]
fn icc_cmyk_overprint_matches_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let directory = env::temp_dir().join(format!("page-icc-cmyk-overprint-{}", std::process::id()));
    fs::create_dir_all(&directory).expect("create fixture directory");
    for (case, expected_failure) in CASES {
        let bytes = common::icc_cmyk_overprint_fixture(case);
        let path = directory.join(format!("{case}.pdf"));
        fs::write(&path, &bytes).expect("write fixture");
        for profile in [ValidationProfile::PdfA2b, ValidationProfile::PdfA3b] {
            assert_eq!(
                validate_pdf_bytes(&bytes, Some(profile), &SafetyLimits::default())
                    .expect("explicit profile validation")
                    .failures
                    .iter()
                    .any(|failure| failure.rule_id.ends_with("ICCBased-CMYK-OVERPRINT-001")),
                *expected_failure,
                "{case}: unexpected local {profile} result"
            );
        }
        for (profile, rule) in [
            (ReferenceProfile::PdfA2b, "ISO 19005-2:2011:6.2.4.2:2"),
            (ReferenceProfile::PdfA3b, "ISO 19005-3:2012:6.2.4.2:2"),
        ] {
            let mut config = ReferenceConfig::pinned(&executable);
            config.profile = profile;
            let result = DifferentialRunner::new(config)
                .expect("pinned veraPDF")
                .compare_file(&path, &SafetyLimits::default())
                .reference_result
                .expect("veraPDF result");
            assert_eq!(
                result
                    .failed_rule_ids
                    .iter()
                    .any(|failed| failed.to_string() == rule),
                *expected_failure,
                "{case}: unexpected veraPDF {profile} result"
            );
        }
    }
    fs::remove_dir_all(directory).expect("remove fixture directory");
}
