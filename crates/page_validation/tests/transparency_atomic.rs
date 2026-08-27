pub mod common;

use std::{env, fs};

use page_validation::differential::{
    ComparisonClassification, DifferentialRunner, ReferenceConfig, ReferenceProfile,
};
use page_validation::{SafetyLimits, validate_pdf_bytes};

const EXTGSTATE_SMASK: &str = "PDFA1B-EXTGSTATE-SMASK-001";
const XOBJECT_SMASK: &str = "PDFA1B-XOBJECT-SMASK-001";
const GROUP: &str = "PDFA1B-TRANSPARENCY-GROUP-001";
const BLEND_MODE: &str = "PDFA1B-EXTGSTATE-BLEND-MODE-001";
const STROKE_ALPHA: &str = "PDFA1B-EXTGSTATE-STROKE-ALPHA-001";
const FILL_ALPHA: &str = "PDFA1B-EXTGSTATE-FILL-ALPHA-001";

const CASES: &[(&str, &[&str])] = &[
    ("extgstate_smask_none", &[]),
    ("extgstate_smask_other", &[EXTGSTATE_SMASK]),
    ("extgstate_smask_dictionary", &[EXTGSTATE_SMASK]),
    ("extgstate_smask_null", &[]),
    ("extgstate_smask_indirect_null", &[EXTGSTATE_SMASK]),
    ("extgstate_bm_normal", &[]),
    ("extgstate_bm_compatible", &[]),
    ("extgstate_bm_multiply", &[BLEND_MODE]),
    ("extgstate_bm_null", &[]),
    ("extgstate_stroke_alpha_one", &[]),
    ("extgstate_stroke_alpha_zero", &[STROKE_ALPHA]),
    ("extgstate_fill_alpha_one", &[]),
    ("extgstate_fill_alpha_zero", &[FILL_ALPHA]),
    ("unused_extgstate_transparency", &[]),
    ("xobject_smask", &[XOBJECT_SMASK]),
    ("xobject_smask_null", &[]),
    ("xobject_smask_indirect_null", &[XOBJECT_SMASK]),
    ("unused_xobject_smask", &[]),
    ("page_transparency_group", &[GROUP]),
    ("page_nontransparency_group", &[]),
    ("form_transparency_group", &[GROUP]),
    ("unused_form_transparency_group", &[]),
];

#[test]
fn transparency_cases_have_the_complete_expected_failure_delta() {
    let baseline = common::failure_ids(&common::graphics_fixture("baseline"));
    for rule in [
        EXTGSTATE_SMASK,
        XOBJECT_SMASK,
        GROUP,
        BLEND_MODE,
        STROKE_ALPHA,
        FILL_ALPHA,
    ] {
        assert!(!baseline.contains(rule));
    }

    common::assert_case_deltas(common::graphics_fixture, "baseline", CASES);
}

#[test]
fn a_single_transparency_failure_attaches_its_owner() {
    let report = common::validate(&common::graphics_fixture("extgstate_bm_multiply"));
    let failure = common::assert_single_failure(&report, BLEND_MODE);
    assert!(failure.object_id.is_some());
}

#[test]
fn pdfa_2_requires_page_group_cs_for_used_transparency_without_output_intent() {
    for (profile, rule_id) in [
        (
            page_validation::ValidationProfile::PdfA2b,
            "PDFA2B-TRANSPARENCY-GROUP-CS-001",
        ),
        (
            page_validation::ValidationProfile::PdfA3b,
            "PDFA3B-TRANSPARENCY-GROUP-CS-001",
        ),
    ] {
        let report = validate_pdf_bytes(
            &common::graphics_fixture("extgstate_transparency_no_output_intent"),
            Some(profile),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert!(
            report
                .failures
                .iter()
                .any(|failure| failure.rule_id == rule_id),
            "{profile}: {report}"
        );
    }
}

#[test]
fn transparency_group_cs_matches_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let path = env::temp_dir().join(format!(
        "page-transparency-group-cs-{}.pdf",
        std::process::id()
    ));
    fs::write(
        &path,
        common::graphics_fixture("extgstate_transparency_no_output_intent"),
    )
    .expect("write transparency fixture");
    for (profile, expected_rule) in [
        (ReferenceProfile::PdfA2b, "ISO 19005-2:2011:6.2.10:2"),
        (ReferenceProfile::PdfA3b, "ISO 19005-3:2012:6.2.10:2"),
    ] {
        let mut config = ReferenceConfig::pinned(&executable);
        config.profile = profile;
        let report = DifferentialRunner::new(config)
            .expect("pinned veraPDF")
            .compare_file(&path, &SafetyLimits::default());
        assert_eq!(
            report.classification,
            ComparisonClassification::BothNoncompliant
        );
        let reference = report.reference_result.as_ref().expect("veraPDF result");
        assert!(
            reference
                .failed_rule_ids
                .iter()
                .any(|rule| rule.to_string() == expected_rule),
            "{profile}: {report:?}"
        );
    }
    fs::remove_file(path).expect("remove transparency fixture");
}
