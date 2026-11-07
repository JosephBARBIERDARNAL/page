use lopdf::{Document, Object};
use page_validation::{ValidationOptions, ValidationProfile, validate_pdf_bytes};

pub mod common;

#[test]
fn a_single_composite_font_failure_is_reported() {
    let report = common::validate(&common::font_fixture("composite_cidmap_missing"));
    common::assert_single_failure(&report, "ISO 19005-1:2005:6.3.3.2:1");
}

#[test]
fn cid_fonts_without_w_or_dw_use_the_pdf_default_width() {
    assert_cid_width_findings(None, None, true);
}

#[test]
fn cids_omitted_from_w_use_the_pdf_default_width() {
    assert_cid_width_findings(None, Some(vec![31.into(), 31.into(), 500.into()]), true);
}

#[test]
fn explicit_cid_widths_override_the_pdf_default() {
    for (default_matches, widths, mismatch) in [
        (true, None, false),
        (false, None, true),
        (true, Some(vec![31.into(), 31.into(), 500.into()]), false),
    ] {
        assert_cid_width_findings(Some(default_matches), widths, mismatch);
    }
}

fn assert_cid_width_findings(
    default_matches: Option<bool>,
    widths: Option<Vec<Object>>,
    mismatch: bool,
) {
    for (fixture, subtype, matching_width) in [
        (
            "composite_identity_width_mismatch",
            b"CIDFontType2".as_slice(),
            500,
        ),
        ("composite_cff_present_glyph", b"CIDFontType0".as_slice(), 0),
    ] {
        let mut document = Document::load_mem(&common::font_fixture(fixture))
            .expect("load embedded CID font fixture");
        let descendant = document
            .objects
            .values_mut()
            .filter_map(|object| object.as_dict_mut().ok())
            .find(|dictionary| {
                dictionary.get(b"Subtype").and_then(Object::as_name).ok() == Some(subtype)
            })
            .expect("CID font descendant");
        descendant.remove(b"DW");
        if let Some(matches) = default_matches {
            descendant.set("DW", if matches { matching_width } else { 400 });
        }
        if let Some(widths) = &widths {
            descendant.set("W", Object::Array(widths.clone()));
        }
        let mut bytes = Vec::new();
        document
            .save_to(&mut bytes)
            .expect("save CID width fixture");
        for (profile, rule) in [
            (ValidationProfile::PdfA1b, "ISO 19005-1:2005:6.3.6:1"),
            (ValidationProfile::PdfA2b, "ISO 19005-2:2011:6.2.11.5:1"),
            (ValidationProfile::PdfUa1, "ISO 14289-1:2014:7.21.5:1"),
        ] {
            let report = validate_pdf_bytes(&bytes, &ValidationOptions::default().profile(profile))
                .expect("validate CID width fixture");
            assert_eq!(
                report
                    .failures
                    .iter()
                    .any(|failure| failure.rule_id == rule),
                mismatch,
                "{fixture}, {profile}: {:?}",
                report.failures,
            );
        }
    }
}
