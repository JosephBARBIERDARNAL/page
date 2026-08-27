pub mod common;

use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

const CASES: &[(&str, &[&str])] = &[
    ("image_alternates", &["PDFA1B-IMAGE-ALTERNATES-001"]),
    ("image_alternates_null", &[]),
    ("image_opi", &["PDFA1B-XOBJECT-OPI-001"]),
    ("image_opi_null", &[]),
    ("form_opi", &["PDFA1B-XOBJECT-OPI-001"]),
    ("form_opi_null", &[]),
    ("image_interpolate_true", &["PDFA1B-IMAGE-INTERPOLATE-001"]),
    (
        "image_interpolate_indirect_true",
        &["PDFA1B-IMAGE-INTERPOLATE-001"],
    ),
    ("image_interpolate_false", &[]),
    ("image_interpolate_null", &[]),
    ("image_bpc_16", &["PDFA1B-IMAGE-BPC-001"]),
    ("image_bpc_indirect_16", &["PDFA1B-IMAGE-BPC-001"]),
    ("image_subtype_indirect_bpc_16", &["PDFA1B-IMAGE-BPC-001"]),
    ("image_mask_indirect_true_bpc_16", &["PDFA1B-IMAGE-BPC-001"]),
    ("direct_image_bpc_16", &["PDFA1B-IMAGE-BPC-001"]),
    (
        "indirect_xobject_dictionary_image_bpc_16",
        &["PDFA1B-IMAGE-BPC-001"],
    ),
    ("inherited_xobject_image_bpc_16", &["PDFA1B-IMAGE-BPC-001"]),
    ("image_bpc_missing", &[]),
    ("mask_bpc_8", &[]),
    ("mask_bpc_missing", &[]),
    ("explicit_mask_bpc_8", &["PDFA1B-IMAGE-MASK-BPC-001"]),
    (
        "shared_painted_explicit_mask_bpc_16",
        &["PDFA1B-IMAGE-BPC-001", "PDFA1B-IMAGE-MASK-BPC-001"],
    ),
    ("form_ps_key", &["PDFA1B-FORM-POSTSCRIPT-001"]),
    ("form_ps_null", &[]),
    ("form_subtype2_ps", &["PDFA1B-FORM-POSTSCRIPT-001"]),
    ("form_subtype2_indirect_ps", &["PDFA1B-FORM-POSTSCRIPT-001"]),
    ("form_ref", &["PDFA1B-FORM-REFERENCE-001"]),
    ("direct_form_ref", &["PDFA1B-FORM-REFERENCE-001"]),
    ("form_ref_null", &[]),
    ("postscript_xobject", &["PDFA1B-XOBJECT-POSTSCRIPT-001"]),
    (
        "postscript_subtype_indirect",
        &["PDFA1B-XOBJECT-POSTSCRIPT-001"],
    ),
    (
        "direct_postscript_xobject",
        &["PDFA1B-XOBJECT-POSTSCRIPT-001"],
    ),
    ("unused_resource_invalid_image", &[]),
    ("unreferenced_invalid_image", &[]),
    ("two_invalid_images", &["PDFA1B-IMAGE-BPC-001"]),
];

#[test]
fn xobject_cases_have_the_complete_expected_failure_delta() {
    common::assert_case_deltas(common::xobject_fixture, "baseline", CASES);
}

#[test]
fn multiple_invalid_xobjects_are_one_deterministic_unattached_failure() {
    let report = common::validate(&common::xobject_fixture("two_invalid_images"));
    let failure = common::assert_single_failure(&report, "PDFA1B-IMAGE-BPC-001");
    assert!(failure.object_id.is_none());
    assert!(failure.message.contains("image"));
}

#[test]
fn pdfa_2_and_3_keep_image_and_form_opi_predicates_separate() {
    let image = validate_bytes_with_profile(
        &common::xobject_fixture("image_opi"),
        ValidationProfile::PdfA2b,
        &SafetyLimits::default(),
    );
    assert!(
        image
            .failures
            .iter()
            .any(|failure| failure.rule_id == "PDFA2B-XOBJECT-OPI-001")
    );
    assert!(
        image
            .failures
            .iter()
            .all(|failure| failure.rule_id != "PDFA2B-FORM-POSTSCRIPT-001")
    );

    let form = validate_bytes_with_profile(
        &common::xobject_fixture("form_opi"),
        ValidationProfile::PdfA2b,
        &SafetyLimits::default(),
    );
    assert!(
        form.failures
            .iter()
            .any(|failure| failure.rule_id == "PDFA2B-FORM-POSTSCRIPT-001")
    );
    assert!(
        form.failures
            .iter()
            .all(|failure| failure.rule_id != "PDFA2B-XOBJECT-OPI-001")
    );
}
