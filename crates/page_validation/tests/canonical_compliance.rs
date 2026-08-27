use page_validation::{SafetyLimits, ValidationProfile, validate_bytes};

use std::{env, path::Path};

use page_validation::differential::{
    ComparisonClassification, DifferentialRunner, ReferenceConfig, ReferenceProfile,
};

#[test]
fn canonical_pdfa_1a_is_locally_compliant() {
    let report = validate_bytes(
        include_bytes!("fixtures/canonical-pdfa-1a.pdf"),
        Some(ValidationProfile::PdfA1a),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");

    assert!(report.checks_passed, "{report}");
    assert!(report.failures.is_empty(), "{report}");
}

#[test]
fn canonical_pdfa_1b_is_locally_compliant() {
    let report = validate_bytes(
        include_bytes!("fixtures/canonical-pdfa-1b.pdf"),
        Some(ValidationProfile::PdfA1b),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation");

    assert!(report.checks_passed, "{report}");
    assert!(report.failures.is_empty(), "{report}");
}

#[test]
fn canonical_pdfa_1a_matches_verapdf() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        eprintln!("VERAPDF_BIN is unset; skipping opt-in veraPDF canonical test");
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfA1a;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    let report = runner.compare_file(
        Path::new("tests/fixtures/canonical-pdfa-1a.pdf"),
        &SafetyLimits::default(),
    );

    assert_eq!(
        report.classification,
        ComparisonClassification::Agreement,
        "{report}"
    );
    assert!(report.local_report.failures.is_empty(), "{report}");
    assert_eq!(
        report
            .reference_result
            .as_ref()
            .expect("veraPDF result")
            .compliant,
        Some(true),
        "{report}"
    );
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
        let report = validate_bytes(&bytes, Some(profile), &SafetyLimits::default())
            .expect("explicit profile validation");
        assert!(report.checks_passed, "{profile}: {report}");
        assert!(report.failures.is_empty(), "{profile}: {report}");
        assert_eq!(
            report.checks.total,
            profile.implemented_check_count(),
            "{profile}: {report}"
        );
    }
}

#[test]
fn canonical_pdfa_2_and_3_profiles_match_verapdf() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        eprintln!("VERAPDF_BIN is unset; skipping opt-in veraPDF PDF/A-2/3 test");
        return;
    };
    let cases = [
        (
            "pdfa2a",
            ReferenceProfile::PdfA2a,
            2,
            b'A',
            "canonical-pdfa-1a.pdf",
        ),
        (
            "pdfa2b",
            ReferenceProfile::PdfA2b,
            2,
            b'B',
            "canonical-pdfa-1b.pdf",
        ),
        (
            "pdfa2u",
            ReferenceProfile::PdfA2u,
            2,
            b'U',
            "canonical-pdfa-1a.pdf",
        ),
        (
            "pdfa3a",
            ReferenceProfile::PdfA3a,
            3,
            b'A',
            "canonical-pdfa-1a.pdf",
        ),
        (
            "pdfa3b",
            ReferenceProfile::PdfA3b,
            3,
            b'B',
            "canonical-pdfa-1b.pdf",
        ),
        (
            "pdfa3u",
            ReferenceProfile::PdfA3u,
            3,
            b'U',
            "canonical-pdfa-1a.pdf",
        ),
    ];
    for (name, profile, part, conformance, fixture) in cases {
        let source = match fixture {
            "canonical-pdfa-1a.pdf" => include_bytes!("fixtures/canonical-pdfa-1a.pdf").as_slice(),
            _ => include_bytes!("fixtures/canonical-pdfa-1b.pdf").as_slice(),
        };
        let path = env::temp_dir().join(format!("page-{name}-{}-{part}.pdf", std::process::id()));
        std::fs::write(&path, reidentify(source, part, conformance)).expect("write fixture");
        let mut config = ReferenceConfig::pinned(&executable);
        config.profile = profile;
        let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
        let report = runner.compare_file(&path, &SafetyLimits::default());
        std::fs::remove_file(&path).expect("remove temporary fixture");
        assert_eq!(
            report.classification,
            ComparisonClassification::Agreement,
            "{report}"
        );
    }
}

#[test]
fn pdfa_2_accepts_pdfa_1_xref_relaxations() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        eprintln!("VERAPDF_BIN is unset; skipping opt-in PDF/A-2 relaxation test");
        return;
    };
    for (name, fixture) in [
        ("xref-spacing", "xref-spacing.pdf"),
        ("xref-stream", "xref-stream.pdf"),
    ] {
        let source = match fixture {
            "xref-spacing.pdf" => include_bytes!("fixtures/xref-spacing.pdf").as_slice(),
            _ => include_bytes!("fixtures/xref-stream.pdf").as_slice(),
        };
        let bytes = reidentify(source, 2, b'B');
        let local = validate_bytes(
            &bytes,
            Some(ValidationProfile::PdfA2b),
            &SafetyLimits::default(),
        )
        .expect("explicit profile validation");
        assert!(local.checks_passed, "{name}: {local}");
        let path = env::temp_dir().join(format!("page-pdfa2-{name}-{}.pdf", std::process::id()));
        std::fs::write(&path, bytes).expect("write temporary PDF/A-2 fixture");
        let mut config = ReferenceConfig::pinned(executable.clone());
        config.profile = ReferenceProfile::PdfA2b;
        let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
        let report = runner.compare_file(&path, &SafetyLimits::default());
        std::fs::remove_file(&path).expect("remove temporary PDF/A-2 fixture");
        assert_eq!(
            report.classification,
            ComparisonClassification::Agreement,
            "{report}"
        );
    }
}
