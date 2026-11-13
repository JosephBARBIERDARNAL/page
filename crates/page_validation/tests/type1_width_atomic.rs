use lopdf::{Document, Object, Stream, dictionary};
use page_validation::{ValidationOptions, ValidationProfile, validate_pdf_bytes};

pub mod common;

#[test]
fn type1_binary_whitespace_preserves_matching_and_mismatched_widths() {
    for first in [b' ', b'\t', b'\r', b'\n', 0x0b, 0x0c, 0, b'!'] {
        for width in [400, 556] {
            let program = common::type1_program_with_width_and_first_ciphertext_byte(width, first);
            assert_width_failure(&program, width != 400);
        }
    }
}

#[test]
fn unavailable_type1_width_is_not_compared_as_zero() {
    assert_width_failure(&common::type1_program(&["space"]), false);
}

#[test]
fn decoded_zero_type1_width_is_still_compared() {
    assert_width_failure(&common::type1_program_with_width(0), true);
}

fn assert_width_failure(program: &[u8], mismatch: bool) {
    let mut document = Document::load_mem(&common::font_fixture("type1_width_mismatch"))
        .expect("load Type 1 font fixture");
    let program_id = document
        .objects
        .values()
        .filter_map(|object| object.as_dict().ok())
        .find_map(|dictionary| dictionary.get(b"FontFile").ok())
        .and_then(|object| object.as_reference().ok())
        .expect("Type 1 font program reference");
    let length1 = program
        .windows(6)
        .position(|window| window == b"eexec\n")
        .expect("eexec boundary")
        + 6;
    document.objects.insert(
        program_id,
        Object::Stream(Stream::new(
            dictionary! {
                "Length1" => length1 as i64,
                "Length2" => (program.len() - length1) as i64,
                "Length3" => 0,
            },
            program.to_vec(),
        )),
    );
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).expect("save Type 1 fixture");
    for (profile, rule) in [
        (ValidationProfile::PdfA1b, "ISO 19005-1:2005:6.3.6:1"),
        (ValidationProfile::PdfA2b, "ISO 19005-2:2011:6.2.11.5:1"),
        (ValidationProfile::PdfUa1, "ISO 14289-1:2014:7.21.5:1"),
    ] {
        let report = validate_pdf_bytes(&bytes, &ValidationOptions::default().profile(profile))
            .expect("validate Type 1 widths");
        assert_eq!(
            report
                .failures
                .iter()
                .any(|failure| failure.rule_id == rule),
            mismatch,
            "{profile}: {:?}",
            report.failures
        );
    }
}
