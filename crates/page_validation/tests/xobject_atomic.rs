#[allow(dead_code)]
mod common;

const CASES: &[(&str, &[&str])] = &[
    ("image_alternates", &["PDFA1B-IMAGE-ALTERNATES-001"]),
    ("image_alternates_null", &[]),
    ("image_opi", &["PDFA1B-XOBJECT-OPI-001"]),
    ("image_opi_null", &[]),
    ("form_opi", &["PDFA1B-XOBJECT-OPI-001"]),
    ("form_opi_null", &[]),
    ("image_interpolate_true", &["PDFA1B-IMAGE-INTERPOLATE-001"]),
    ("image_interpolate_false", &[]),
    ("image_interpolate_null", &[]),
    ("image_bpc_16", &["PDFA1B-IMAGE-BPC-001"]),
    ("image_bpc_missing", &[]),
    ("mask_bpc_8", &[]),
    ("mask_bpc_missing", &[]),
    ("explicit_mask_bpc_8", &["PDFA1B-IMAGE-MASK-BPC-001"]),
    ("form_ps_key", &["PDFA1B-FORM-POSTSCRIPT-001"]),
    ("form_ps_null", &[]),
    ("form_subtype2_ps", &["PDFA1B-FORM-POSTSCRIPT-001"]),
    ("form_ref", &["PDFA1B-FORM-REFERENCE-001"]),
    ("form_ref_null", &[]),
    ("postscript_xobject", &["PDFA1B-XOBJECT-POSTSCRIPT-001"]),
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
    assert!(failure.message.contains("; "));
}
