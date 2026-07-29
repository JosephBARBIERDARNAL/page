use std::collections::BTreeSet;

use mai_validation::{SafetyLimits, ValidationProfile, validate_bytes};

#[allow(dead_code)]
mod common;

const CASES: &[(&str, &[&str])] = &[
    ("image_alternates", &["PDFA1B-IMAGE-ALTERNATES-001"]),
    ("image_opi", &["PDFA1B-XOBJECT-OPI-001"]),
    ("form_opi", &["PDFA1B-XOBJECT-OPI-001"]),
    ("image_interpolate_true", &["PDFA1B-IMAGE-INTERPOLATE-001"]),
    ("image_interpolate_false", &[]),
    ("image_bpc_16", &["PDFA1B-IMAGE-BPC-001"]),
    ("image_bpc_missing", &[]),
    ("mask_bpc_8", &[]),
    ("mask_bpc_missing", &[]),
    ("explicit_mask_bpc_8", &["PDFA1B-IMAGE-MASK-BPC-001"]),
    ("form_ps_key", &["PDFA1B-FORM-POSTSCRIPT-001"]),
    ("form_ps_null", &[]),
    ("form_subtype2_ps", &["PDFA1B-FORM-POSTSCRIPT-001"]),
    ("form_ref", &["PDFA1B-FORM-REFERENCE-001"]),
    ("postscript_xobject", &["PDFA1B-XOBJECT-POSTSCRIPT-001"]),
    ("unused_resource_invalid_image", &[]),
    ("unreferenced_invalid_image", &[]),
    ("two_invalid_images", &["PDFA1B-IMAGE-BPC-001"]),
];

#[test]
fn xobject_cases_have_the_complete_expected_failure_delta() {
    let baseline = failure_ids(&common::xobject_fixture("baseline"));
    for (case, expected_added) in CASES {
        let actual = failure_ids(&common::xobject_fixture(case));
        let (added, removed) = common::rule_delta(&baseline, &actual);
        let expected_added = expected_added
            .iter()
            .map(|rule_id| (*rule_id).to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(added, expected_added, "{case}: unexpected added failures");
        assert!(
            removed.is_empty(),
            "{case}: removed baseline failures {removed:?}"
        );
    }
}

#[test]
fn multiple_invalid_xobjects_are_one_deterministic_unattached_failure() {
    let report = validate_bytes(
        &common::xobject_fixture("two_invalid_images"),
        ValidationProfile::PdfA1b,
        &SafetyLimits::default(),
    );
    let failure = report
        .failures
        .iter()
        .find(|failure| failure.rule_id == "PDFA1B-IMAGE-BPC-001")
        .expect("image bit-depth failure");
    assert!(failure.object_id.is_none());
    assert!(failure.message.contains("; "));
    assert_eq!(report.checks.total, 99);
    assert_eq!(report.checks.failed, 1);
    assert_eq!(report.checks.passed, 98);
}

fn failure_ids(bytes: &[u8]) -> BTreeSet<String> {
    validate_bytes(bytes, ValidationProfile::PdfA1b, &SafetyLimits::default())
        .failures
        .into_iter()
        .map(|failure| failure.rule_id.to_owned())
        .collect()
}
