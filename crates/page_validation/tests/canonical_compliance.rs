use page_validation::{SafetyLimits, ValidationProfile, validate_pdf_bytes};

#[test]
fn canonical_pdfa_1a_is_locally_compliant() {
    let report = validate_pdf_bytes(
        include_bytes!("fixtures/canonical-pdfa-1a.pdf"),
        Some(ValidationProfile::PdfA1a),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");

    assert!(report.is_compliant, "{report}");
    assert!(report.failures.is_empty(), "{report}");
}

#[test]
fn canonical_pdfa_1b_is_locally_compliant() {
    let report = validate_pdf_bytes(
        include_bytes!("fixtures/canonical-pdfa-1b.pdf"),
        Some(ValidationProfile::PdfA1b),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");

    assert!(report.is_compliant, "{report}");
    assert!(report.failures.is_empty(), "{report}");
}

fn reidentify(bytes: &[u8], part: u8, conformance: u8) -> Vec<u8> {
    let mut bytes = bytes.to_vec();
    let part_marker = b"<pdfaid:part>1</pdfaid:part>";
    let part_replacement = format!("<pdfaid:part>{part}</pdfaid:part>");
    replace_once(&mut bytes, part_marker, part_replacement.as_bytes());
    let conformance_marker = if bytes
        .windows(b"<pdfaid:conformance>A</pdfaid:conformance>".len())
        .any(|window| window == b"<pdfaid:conformance>A</pdfaid:conformance>")
    {
        b"<pdfaid:conformance>A</pdfaid:conformance>".as_slice()
    } else {
        b"<pdfaid:conformance>B</pdfaid:conformance>".as_slice()
    };
    let conformance_replacement = format!(
        "<pdfaid:conformance>{}</pdfaid:conformance>",
        char::from(conformance)
    );
    replace_once(
        &mut bytes,
        conformance_marker,
        conformance_replacement.as_bytes(),
    );
    bytes
}

fn replace_once(bytes: &mut [u8], needle: &[u8], replacement: &[u8]) {
    assert_eq!(needle.len(), replacement.len());
    let start = bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("canonical fixture marker");
    bytes
        .get_mut(start..start + needle.len())
        .expect("canonical replacement range")
        .copy_from_slice(replacement);
}

#[test]
fn canonical_pdfa_2_and_3_profiles_are_locally_compliant() {
    let cases = [
        (
            ValidationProfile::PdfA2a,
            reidentify(include_bytes!("fixtures/canonical-pdfa-1a.pdf"), 2, b'A'),
        ),
        (
            ValidationProfile::PdfA2b,
            reidentify(include_bytes!("fixtures/canonical-pdfa-1b.pdf"), 2, b'B'),
        ),
        (
            ValidationProfile::PdfA2u,
            reidentify(include_bytes!("fixtures/canonical-pdfa-1a.pdf"), 2, b'U'),
        ),
        (
            ValidationProfile::PdfA3a,
            reidentify(include_bytes!("fixtures/canonical-pdfa-1a.pdf"), 3, b'A'),
        ),
        (
            ValidationProfile::PdfA3b,
            reidentify(include_bytes!("fixtures/canonical-pdfa-1b.pdf"), 3, b'B'),
        ),
        (
            ValidationProfile::PdfA3u,
            reidentify(include_bytes!("fixtures/canonical-pdfa-1a.pdf"), 3, b'U'),
        ),
    ];
    for (profile, bytes) in cases {
        let report = validate_pdf_bytes(&bytes, Some(profile), &SafetyLimits::default())
            .expect("explicit profile validation");
        assert!(report.is_compliant, "{profile}: {report}");
        assert!(report.failures.is_empty(), "{profile}: {report}");
        assert_eq!(
            report.rules.total,
            profile.implemented_check_count(),
            "{profile}: {report}"
        );
    }
}

#[test]
fn pdfa_2_accepts_pdfa_1_xref_relaxations() {
    for (name, fixture) in [
        (
            "xref-spacing",
            include_bytes!("fixtures/xref-spacing.pdf").as_slice(),
        ),
        (
            "xref-stream",
            include_bytes!("fixtures/xref-stream.pdf").as_slice(),
        ),
    ] {
        let bytes = reidentify(fixture, 2, b'B');
        let report = validate_pdf_bytes(
            &bytes,
            Some(ValidationProfile::PdfA2b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert!(report.is_compliant, "{name}: {report}");
    }
}
