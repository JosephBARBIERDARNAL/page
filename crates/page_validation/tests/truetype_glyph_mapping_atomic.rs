use page_validation::{ValidationOptions, ValidationProfile, validate_pdf_bytes};

pub mod common;

#[test]
fn macintosh_only_font_resolves_macroman_bytes() {
    let cmap = common::sfnt::format6_cmap(&[(1, 0, &[(135, 1), (222, 2)])]);
    let program = common::sfnt::minimal_truetype_with_cmap_and_metrics(cmap, 3, &[500, 556, 500]);
    for (byte, width) in [(135, 556), (222, 500)] {
        let bytes =
            common::truetype_mapping_fixture(program.clone(), "MacRomanEncoding", byte, width);
        assert_glyph_findings(&bytes, false, false);
    }
}

#[test]
fn unicode_mapping_takes_priority_over_conflicting_macintosh_mapping() {
    for records in [
        vec![(1, 0, &[(161, 1), (176, 2)][..]), (3, 1, &[(176, 1)][..])],
        vec![(3, 1, &[(176, 1)][..]), (1, 0, &[(161, 1), (176, 2)][..])],
    ] {
        let cmap = common::sfnt::format6_cmap(&records);
        let program =
            common::sfnt::minimal_truetype_with_cmap_and_metrics(cmap, 3, &[500, 400, 713]);
        let bytes = common::truetype_mapping_fixture(program, "WinAnsiEncoding", 176, 400);
        assert_glyph_findings(&bytes, false, false);
    }
}

#[test]
fn winansi_bytes_are_translated_when_only_macintosh_mapping_exists() {
    let cmap = common::sfnt::format6_cmap(&[(1, 0, &[(161, 1), (176, 2)])]);
    let program = common::sfnt::minimal_truetype_with_cmap_and_metrics(cmap, 3, &[500, 400, 713]);
    let bytes = common::truetype_mapping_fixture(program, "WinAnsiEncoding", 176, 400);
    assert_glyph_findings(&bytes, false, false);
}

#[test]
fn macintosh_mapping_keeps_real_width_mismatches() {
    let cmap = common::sfnt::format6_cmap(&[(1, 0, &[(135, 1)])]);
    let program = common::sfnt::minimal_truetype_with_cmap_and_metrics(cmap, 2, &[500, 556]);
    let bytes = common::truetype_mapping_fixture(program, "MacRomanEncoding", 135, 500);
    assert_glyph_findings(&bytes, false, true);
}

#[test]
fn macintosh_mapping_keeps_genuinely_missing_glyphs() {
    for (encoding, byte, mac_code) in [
        ("MacRomanEncoding", 135, 225),
        ("WinAnsiEncoding", 176, 176),
    ] {
        let cmap = common::sfnt::format6_cmap(&[(1, 0, &[(mac_code, 1)])]);
        let program = common::sfnt::minimal_truetype_with_cmap_and_metrics(cmap, 2, &[500, 500]);
        let bytes = common::truetype_mapping_fixture(program, encoding, byte, 500);
        assert_glyph_findings(&bytes, true, false);
    }
}

#[test]
fn missing_unicode_glyph_does_not_fall_back_to_macintosh_mapping() {
    let cmap = common::sfnt::format6_cmap(&[(1, 0, &[(161, 1), (176, 1)]), (3, 1, &[(177, 1)])]);
    let program = common::sfnt::minimal_truetype_with_cmap_and_metrics(cmap, 2, &[500, 400]);
    let bytes = common::truetype_mapping_fixture(program, "WinAnsiEncoding", 176, 400);
    assert_glyph_findings(&bytes, true, false);
}

fn assert_glyph_findings(bytes: &[u8], missing: bool, width_mismatch: bool) {
    for (profile, missing_rule, notdef_rule, width_rule) in [
        (
            ValidationProfile::PdfA1b,
            "ISO 19005-1:2005:6.3.5:1",
            None,
            "ISO 19005-1:2005:6.3.6:1",
        ),
        (
            ValidationProfile::PdfA2b,
            "ISO 19005-2:2011:6.2.11.4.1:2",
            Some("ISO 19005-2:2011:6.2.11.8:1"),
            "ISO 19005-2:2011:6.2.11.5:1",
        ),
        (
            ValidationProfile::PdfA3b,
            "ISO 19005-3:2012:6.2.11.4.1:2",
            Some("ISO 19005-3:2012:6.2.11.8:1"),
            "ISO 19005-3:2012:6.2.11.5:1",
        ),
        (
            ValidationProfile::PdfUa1,
            "ISO 14289-1:2014:7.21.4.1:2",
            None,
            "ISO 14289-1:2014:7.21.5:1",
        ),
    ] {
        let report = validate_pdf_bytes(bytes, &ValidationOptions::default().profile(profile))
            .expect("validate glyph mapping fixture");
        let has_rule = |rule| {
            report
                .failures
                .iter()
                .any(|failure| failure.rule_id == rule)
        };
        assert_eq!(
            has_rule(missing_rule),
            missing,
            "{profile}: {:?}",
            report.failures
        );
        assert_eq!(
            has_rule(width_rule),
            width_mismatch,
            "{profile}: {:?}",
            report.failures
        );
        if let Some(rule) = notdef_rule {
            assert_eq!(has_rule(rule), missing, "{profile}: {:?}", report.failures);
        }
    }
}
