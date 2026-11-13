pub mod common;

use std::io::Write as _;

use page_validation::{ValidationOptions, ValidationProfile, validate_pdf_bytes};

const HEX_CHARACTERS: &str = "ISO 19005-1:2005:6.1.6:2";

#[test]
fn recoverable_hex_syntax_is_a_conformance_failure_not_a_parser_failure() {
    let report = common::validate(&common::syntax_fixture("hex_string_invalid_character"));
    assert!(
        report
            .failures
            .iter()
            .any(|failure| failure.rule_id == HEX_CHARACTERS)
    );
    assert!(
        report
            .failures
            .iter()
            .all(|failure| failure.rule_id != "PDF-PARSE-001")
    );
}

#[test]
fn header_suffixes_preserve_original_conformance_findings() {
    for (profile, header_rule, binary_rule) in [
        (
            ValidationProfile::PdfA1b,
            "ISO 19005-1:2005:6.1.2:1",
            Some("ISO 19005-1:2005:6.1.2:2"),
        ),
        (
            ValidationProfile::PdfA2b,
            "ISO 19005-2:2011:6.1.2:1",
            Some("ISO 19005-2:2011:6.1.2:2"),
        ),
        (ValidationProfile::PdfUa1, "ISO 14289-1:2014:6.1:1", None),
    ] {
        for suffix in [
            b"".as_slice(),
            b" ",
            b"X",
            b" Sharp Scanned ImagePDF",
            b"%suffix",
            b"\xff\xfe",
        ] {
            for comment in [b"%\xe2\xe3\xcf\xd3".as_slice(), b"%Sharp Non-Encryption"] {
                let bytes = header_pdf(suffix, comment);
                let report =
                    validate_pdf_bytes(&bytes, &ValidationOptions::default().profile(profile))
                        .expect("a header suffix must not prevent document inspection");
                let has_rule = |rule| {
                    report
                        .failures
                        .iter()
                        .any(|failure| failure.rule_id == rule)
                };
                assert_eq!(
                    has_rule(header_rule),
                    !suffix.is_empty(),
                    "{profile:?}, {suffix:?}"
                );
                if let Some(binary_rule) = binary_rule {
                    assert_eq!(
                        has_rule(binary_rule),
                        comment == b"%Sharp Non-Encryption",
                        "{profile:?}, {suffix:?}"
                    );
                }
            }
        }
    }
}

fn header_pdf(suffix: &[u8], comment: &[u8]) -> Vec<u8> {
    let mut bytes = b"%PDF-1.4".to_vec();
    bytes.extend_from_slice(suffix);
    bytes.push(b'\n');
    bytes.extend_from_slice(comment);
    bytes.push(b'\n');
    let mut offsets = Vec::new();
    for (index, object) in [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Resources << >> >>",
    ]
    .iter()
    .enumerate()
    {
        offsets.push(bytes.len());
        writeln!(bytes, "{} 0 obj\n{object}\nendobj", index + 1).expect("write object");
    }
    let xref_offset = bytes.len();
    bytes.extend_from_slice(b"xref\n0 4\n0000000000 65535 f \n");
    for offset in offsets {
        writeln!(bytes, "{offset:010} 00000 n ").expect("write xref entry");
    }
    writeln!(
        bytes,
        "trailer\n<< /Size 4 /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF"
    )
    .expect("write trailer");
    bytes
}
