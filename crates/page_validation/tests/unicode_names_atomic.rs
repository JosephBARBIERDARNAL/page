use std::{env, fs};

use lopdf::Object;
use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::SafetyLimits;

pub mod common;

const CASES: &[(&str, fn(&str) -> Vec<u8>)] = &[
    ("basefont", common::font_fixture),
    ("basefont_unused", common::font_fixture),
    ("basefont_indirect", common::font_fixture),
    ("separation", common::device_color_fixture),
    ("devicen", common::device_color_fixture),
    ("structure", common::document_feature_fixture),
];

#[test]
fn utf8_name_population_matches_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let directory = env::temp_dir().join(format!("page-unicode-names-{}", std::process::id()));
    fs::create_dir_all(&directory).expect("create Unicode-name fixture directory");
    for (case, fixture) in CASES {
        let fixture_case = match *case {
            "basefont" => "unicode_name_basefont_invalid",
            "basefont_unused" => "unicode_name_basefont_unused",
            "basefont_indirect" => "unicode_name_basefont_indirect",
            "separation" => "separation_invalid_utf8",
            "devicen" => "devicen_invalid_utf8",
            "structure" => "unicode_name_structure_invalid",
            _ => unreachable!("known Unicode-name fixture"),
        };
        let path = directory.join(format!("{case}.pdf"));
        fs::write(&path, fixture(fixture_case)).expect("write Unicode-name fixture");
        for (profile, expected_rule) in [
            (ReferenceProfile::PdfA2b, "ISO 19005-2:2011:6.1.8:1"),
            (ReferenceProfile::PdfA3b, "ISO 19005-3:2012:6.1.8:1"),
        ] {
            let mut config = ReferenceConfig::pinned(&executable);
            config.profile = profile;
            let report = DifferentialRunner::new(config)
                .expect("pinned veraPDF")
                .compare_file(&path, &SafetyLimits::default());
            assert!(
                report
                    .reference_result
                    .expect("veraPDF result")
                    .failed_rule_ids
                    .iter()
                    .any(|rule| rule.to_string() == expected_rule),
                "{case}: {profile} did not report its Unicode-name rule"
            );
        }
    }
    fs::remove_dir_all(directory).expect("remove Unicode-name fixture directory");
}

#[test]
fn escaped_name_fixture_expands_to_the_original_invalid_bytes() {
    let bytes = common::font_fixture("unicode_name_basefont_invalid");
    assert!(bytes
        .windows(b"/MaiTest#FFFont".len())
        .any(|window| window == b"/MaiTest#FFFont"));
    let document = lopdf::Document::load_mem(&bytes).expect("parse escaped-name fixture");
    assert!(document.objects.values().any(|object| {
        matches!(
            object,
            Object::Dictionary(dictionary)
                if dictionary
                    .get(b"BaseFont")
                    .ok()
                    .and_then(|value| value.as_name().ok())
                    == Some(b"MaiTest\xffFont".as_slice())
        )
    }));
}
