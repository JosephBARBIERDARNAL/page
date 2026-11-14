use lopdf::{Dictionary, Document, Stream};
use page_validation::{ValidationOptions, ValidationProfile, validate_pdf_bytes};

pub mod common;

#[test]
fn unused_invalid_destination_preserves_used_unicode_mapping() {
    for unused in ["0042", "D800", "DC00", "41"] {
        let cmap = format!("2 beginbfchar <0041> <0041> <0042> <{unused}> endbfchar");
        assert_unicode_findings(&cmap, "0041", false, false);
    }
}

#[test]
fn used_invalid_destination_fails_unicode_mapping() {
    for invalid in ["D800", "DC00", "41"] {
        let cmap = format!("2 beginbfchar <0041> <0041> <0042> <{invalid}> endbfchar");
        assert_unicode_findings(&cmap, "0042", true, false);
    }
}

#[test]
fn missing_used_mapping_still_fails_with_an_invalid_unused_destination() {
    assert_unicode_findings(
        "2 beginbfchar <0041> <0041> <0042> <D800> endbfchar",
        "0043",
        true,
        false,
    );
}

#[test]
fn reserved_values_are_detected_alongside_invalid_unused_destinations() {
    for reserved in ["0000", "FEFF", "FFFE"] {
        let cmap =
            format!("3 beginbfchar <0041> <0041> <0042> <D800> <0043> <{reserved}> endbfchar");
        assert_unicode_findings(&cmap, "0041", false, true);
    }
}

#[test]
fn malformed_cmap_syntax_still_fails_unicode_mapping() {
    for cmap in [
        "2 beginbfchar <0041> <0041> <0042> endbfchar",
        "2 beginbfchar <0041> <0041> <0042> <ZZZZ> endbfchar",
        "1 beginbfrange <0041> <0042> [<0041>] endbfrange",
    ] {
        assert_unicode_findings(cmap, "0041", true, false);
    }
}

fn assert_unicode_findings(cmap: &str, shown_code: &str, invalid: bool, reserved: bool) {
    let bytes = unicode_destination_fixture(cmap, shown_code);
    for (profile, mapping_rule, reserved_rule) in [
        (ValidationProfile::PdfA1a, "ISO 19005-1:2005:6.3.8:1", None),
        (
            ValidationProfile::PdfA2u,
            "ISO 19005-2:2011:6.2.11.7.2:1",
            Some("ISO 19005-2:2011:6.2.11.7.2:2"),
        ),
        (
            ValidationProfile::PdfA3u,
            "ISO 19005-3:2012:6.2.11.7.2:1",
            Some("ISO 19005-3:2012:6.2.11.7.2:2"),
        ),
        (
            ValidationProfile::PdfUa1,
            "ISO 14289-1:2014:7.21.7:1",
            Some("ISO 14289-1:2014:7.21.7:2"),
        ),
    ] {
        let report = validate_pdf_bytes(&bytes, &ValidationOptions::default().profile(profile))
            .expect("validate Unicode destination fixture");
        let has_rule = |rule| {
            report
                .failures
                .iter()
                .any(|failure| failure.rule_id == rule)
        };
        assert_eq!(has_rule(mapping_rule), invalid, "{profile}: {cmap}");
        if let Some(rule) = reserved_rule {
            assert_eq!(has_rule(rule), reserved, "{profile}: {cmap}");
        }
    }
}

fn unicode_destination_fixture(cmap: &str, shown_code: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&common::unembedded_type0_cmap_fixture(
        "Identity-H",
        "Identity",
    ))
    .expect("load Type0 font fixture");
    let to_unicode = document.add_object(Stream::new(
        Dictionary::new(),
        format!("1 begincodespacerange <0000> <FFFF> endcodespacerange {cmap}").into_bytes(),
    ));
    for object in document.objects.values_mut() {
        if let Ok(font) = object.as_dict_mut()
            && font
                .get(b"Subtype")
                .and_then(|value| value.as_name())
                .is_ok_and(|name| name == b"Type0")
        {
            font.set("ToUnicode", to_unicode);
        }
        if let Ok(stream) = object.as_stream_mut()
            && stream.content.starts_with(b"BT /F1")
        {
            stream
                .set_content(format!("BT /F1 12 Tf 20 50 Td <{shown_code}> Tj ET\n").into_bytes());
        }
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save Unicode destination fixture");
    bytes
}
