use std::collections::BTreeSet;
use std::{env, fs};

use page_validation::differential::{DifferentialRunner, ReferenceConfig, ReferenceProfile};
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

pub mod common;

const RULE: &str = "PDFA1A-UNICODE-MAPPING-001";
const PDF2_VALUE_RULE: &str = "PDFA2A-UNICODE-VALUE-001";

const INVALID_CASES: &[&str] = &[
    "unicode_missing",
    "unicode_scalar",
    "unicode_malformed",
    "unicode_incomplete",
];

const EXCEPTION_CASES: &[&str] = &[
    "unicode_winansi",
    "unicode_macroman",
    "unicode_macexpert",
    "unicode_indirect",
    "unicode_type1_standard",
    "unicode_type1_symbol",
    "unicode_type0_identity_h",
    "unicode_type0_identity_v",
    "unicode_type0_gb1",
];

fn a1_failures(case: &str) -> BTreeSet<String> {
    validate_bytes_with_profile(
        &common::font_fixture(case),
        ValidationProfile::PdfA1a,
        &SafetyLimits::default(),
    )
    .failures
    .into_iter()
    .map(|failure| failure.rule_id.to_owned())
    .collect()
}

#[test]
fn rendered_fonts_require_usable_unicode_mappings_in_pdfa_1a() {
    for case in INVALID_CASES {
        let failures = a1_failures(case);
        assert!(failures.contains(RULE), "{case}: {failures:?}");
    }
    for case in EXCEPTION_CASES {
        let failures = a1_failures(case);
        assert!(!failures.contains(RULE), "{case}: {failures:?}");
    }
}

#[test]
fn unicode_mapping_rule_is_profile_specific() {
    for case in INVALID_CASES {
        assert!(
            !common::failure_ids(&common::font_fixture(case)).contains(RULE),
            "PDF/A-1b must not report {RULE} for {case}"
        );
    }
}

#[test]
fn pdfa2_rejects_reserved_tounicode_values() {
    let report = validate_bytes_with_profile(
        &common::font_fixture("unicode_reserved"),
        ValidationProfile::PdfA2a,
        &SafetyLimits::default(),
    );
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.rule_id == PDF2_VALUE_RULE),
        "reserved Unicode value was not rejected: {:?}",
        report
            .failures
            .iter()
            .map(|failure| &failure.rule_id)
            .collect::<Vec<_>>()
    );
}

#[test]
fn unicode_mapping_fixtures_are_processable_by_pinned_verapdf() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfA1a;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    let directory = env::temp_dir().join(format!("page-unicode-mapping-{}", std::process::id()));
    fs::create_dir_all(&directory).expect("create differential fixture directory");
    for case in INVALID_CASES.iter().chain(EXCEPTION_CASES) {
        let path = directory.join(format!("{case}.pdf"));
        fs::write(&path, common::font_fixture(case)).expect("write Unicode fixture");
        let report = runner.compare_file(&path, &SafetyLimits::default());
        let reference = report.reference_result.as_ref().expect("reference result");
        let reference_failed = reference
            .failed_rule_ids
            .iter()
            .any(|rule| rule.to_string() == "ISO 19005-1:2005:6.3.8:1");
        // veraPDF 1.30.2 honors the simple-font and Adobe collection
        // exceptions, but reports 6.3.8.1 for the two predefined Identity
        // CMap cases despite the exception stated in its pinned profile
        // description. These minimal fixtures are the upstream reprex.
        let expected_reference_failure = INVALID_CASES.contains(case)
            || matches!(
                case,
                &"unicode_type0_identity_h" | &"unicode_type0_identity_v"
            );
        assert_eq!(
            reference_failed, expected_reference_failure,
            "{case}: {report}"
        );
        assert!(report.operational_failure.is_none(), "{case}: {report}");
        fs::remove_file(path).expect("remove Unicode fixture");
    }
    fs::remove_dir(directory).expect("remove differential fixture directory");
}
