#![expect(
    clippy::panic,
    reason = "shared fixture dispatchers deliberately fail loudly for undeclared test cases"
)]

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use lopdf::content::{Content, Operation};
use lopdf::xref::XrefType;
use lopdf::{
    Dictionary, Document, EncryptionState, EncryptionVersion, Object, ObjectId, Permissions,
    Stream, StringFormat, dictionary,
};
use page_validation::differential::{
    ComparisonClassification, DifferentialRunner, ReferenceConfig, ReferenceProfile,
};
use page_validation::{
    SafetyLimits, ValidationFailure, ValidationProfile, ValidationReport, validate_pdf_bytes,
};

pub mod sfnt;

pub fn pdf_document() -> Document {
    let mut document = Document::with_version("1.4");
    document.reference_table.cross_reference_type = XrefType::CrossReferenceTable;
    document.trailer.set(
        "ID",
        vec![
            Object::string_literal("0123456789abcdef"),
            Object::string_literal("0123456789abcdef"),
        ],
    );
    document
}

/// Splits `actual` against `baseline` into (added, removed) sets.
pub fn rule_delta<T: Ord + Clone>(
    baseline: &BTreeSet<T>,
    actual: &BTreeSet<T>,
) -> (BTreeSet<T>, BTreeSet<T>) {
    (
        actual.difference(baseline).cloned().collect(),
        baseline.difference(actual).cloned().collect(),
    )
}

pub fn validate(bytes: &[u8]) -> ValidationReport {
    validate_pdf_bytes(
        bytes,
        Some(ValidationProfile::PdfA1b),
        &SafetyLimits::default(),
    )
    .expect("explicit profile validation")
}

pub fn failure_ids(bytes: &[u8]) -> BTreeSet<String> {
    validate(bytes)
        .failures
        .into_iter()
        .map(|failure| failure.rule_id.to_owned())
        .collect()
}

/// Asserts that `report` has exactly one failure, that it is `rule_id`, and
/// that the remaining implemented checks passed. Returns the
/// matching failure so callers can assert further on it (e.g. `object_id`).
pub fn assert_single_failure<'a>(
    report: &'a ValidationReport,
    rule_id: &str,
) -> &'a ValidationFailure {
    let total = ValidationProfile::PdfA1b.implemented_check_count();
    assert_eq!(report.checks.total, total);
    assert_eq!(report.checks.failed, 1);
    assert_eq!(report.checks.passed, total - 1);
    report
        .failures
        .iter()
        .find(|failure| failure.rule_id == rule_id)
        .unwrap_or_else(|| panic!("expected failure {rule_id} not found"))
}

/// Asserts that, relative to `fixture(baseline_case)`'s failures, each
/// `(case, expected_added_rule_ids)` in `cases` adds exactly those rule IDs
/// and removes none of the baseline's failures.
pub fn assert_case_deltas(
    fixture: fn(&str) -> Vec<u8>,
    baseline_case: &str,
    cases: &[(&str, &[&str])],
) {
    let baseline = failure_ids(&fixture(baseline_case));
    for (case, expected_added) in cases {
        let actual = failure_ids(&fixture(case));
        let (added, removed) = rule_delta(&baseline, &actual);
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

/// Returns the canonical compliant fixture appropriate for a PDF/A profile.
pub fn canonical_pdfa_fixture(profile: ReferenceProfile) -> &'static [u8] {
    match profile {
        ReferenceProfile::PdfA1a
        | ReferenceProfile::PdfA2a
        | ReferenceProfile::PdfA2u
        | ReferenceProfile::PdfA3a
        | ReferenceProfile::PdfA3u => include_bytes!("../fixtures/canonical-pdfa-1a.pdf"),
        ReferenceProfile::PdfA1b | ReferenceProfile::PdfA2b | ReferenceProfile::PdfA3b => {
            include_bytes!("../fixtures/canonical-pdfa-1b.pdf")
        }
        ReferenceProfile::PdfUa1 => panic!("PDF/UA is not a PDF/A rule profile"),
    }
}

/// Re-identifies a PDF/A-1 fixture for a PDF/A-2 or PDF/A-3 profile.
pub fn pdfa_profile_fixture(profile: ReferenceProfile, source: &[u8]) -> Vec<u8> {
    if matches!(profile, ReferenceProfile::PdfA1a | ReferenceProfile::PdfA1b) {
        return source.to_vec();
    }
    let (part, conformance) = match profile {
        ReferenceProfile::PdfA2a => (b'2', b'A'),
        ReferenceProfile::PdfA2b => (b'2', b'B'),
        ReferenceProfile::PdfA2u => (b'2', b'U'),
        ReferenceProfile::PdfA3a => (b'3', b'A'),
        ReferenceProfile::PdfA3b => (b'3', b'B'),
        ReferenceProfile::PdfA3u => (b'3', b'U'),
        ReferenceProfile::PdfA1a | ReferenceProfile::PdfA1b | ReferenceProfile::PdfUa1 => {
            panic!("PDF/UA is not a PDF/A-2 or PDF/A-3 profile")
        }
    };
    let mut bytes = source.to_vec();
    replace_pdfa_value(&mut bytes, b"<pdfaid:part>", b"pdfaid:part=\"", part);
    replace_pdfa_value(
        &mut bytes,
        b"<pdfaid:conformance>",
        b"pdfaid:conformance=\"",
        conformance,
    );
    bytes
}

fn replace_pdfa_value(
    bytes: &mut [u8],
    tag_marker: &[u8],
    attribute_marker: &[u8],
    replacement: u8,
) {
    let marker = if bytes
        .windows(tag_marker.len())
        .any(|window| window == tag_marker)
    {
        tag_marker
    } else {
        attribute_marker
    };
    let start = bytes
        .windows(marker.len())
        .position(|window| window == marker)
        .unwrap_or_else(|| {
            panic!(
                "PDF/A marker {:?} not found",
                String::from_utf8_lossy(marker)
            )
        });
    *bytes
        .get_mut(start + marker.len())
        .expect("PDF/A marker value") = replacement;
}

/// Finds the checked-in mutation fixture for a local PDF/A rule, if one exists.
pub fn mutation_fixture(local_rule_id: &str) -> Option<Vec<u8>> {
    let manifest = env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tests/fixtures/verapdf-diff-cases.json");
    let source = fs::read(manifest).expect("read checked-in PDF/A mutation manifest");
    let manifest: serde_json::Value =
        serde_json::from_slice(&source).expect("parse mutation manifest");
    manifest
        .get("checked_in_mutations")
        .and_then(serde_json::Value::as_array)
        .and_then(|mutations| {
            mutations.iter().find(|mutation| {
                mutation
                    .get("local_rule_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(local_rule_id)
            })
        })
        .map(|mutation| {
            let relative = mutation
                .get("path")
                .and_then(serde_json::Value::as_str)
                .expect("mutation path");
            fs::read(
                env::var_os("CARGO_MANIFEST_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(relative),
            )
            .expect("read checked-in PDF/A mutation fixture")
        })
}

/// Selects the profile whose conformance level matches a checked-in mutation.
pub fn preferred_pdfa_mutation_profile(
    local_rule_ids: &[&str],
    profiles: &[ReferenceProfile],
) -> ReferenceProfile {
    let conformance = if local_rule_ids
        .iter()
        .any(|rule_id| rule_id.contains("PDFA1A-"))
    {
        b'A'
    } else {
        b'B'
    };
    profiles
        .iter()
        .copied()
        .find(|profile| {
            matches!(
                (conformance, profile),
                (
                    b'A',
                    ReferenceProfile::PdfA1a | ReferenceProfile::PdfA2a | ReferenceProfile::PdfA3a
                ) | (
                    b'B',
                    ReferenceProfile::PdfA1b | ReferenceProfile::PdfA2b | ReferenceProfile::PdfA3b
                )
            )
        })
        .or_else(|| profiles.first().copied())
        .expect("PDF/A rule has an applicable profile")
}

/// Checks the selected local rule(s) for a valid or invalid fixture without
/// requiring unrelated checks to produce an identical total failure count.
pub fn assert_pdfa_rule_behavior(
    profile: ReferenceProfile,
    local_rule_ids: &[&str],
    bytes: &[u8],
    should_fail: bool,
) {
    let report = validate_pdf_bytes(bytes, Some(profile.into()), &SafetyLimits::default())
        .expect("explicit PDF/A profile validation");
    let failures = report
        .failures
        .iter()
        .map(|failure| failure.rule_id.clone())
        .collect::<BTreeSet<_>>();
    let target_failed = local_rule_ids
        .iter()
        .map(|rule_id| pdfa_profile_local_rule_id(profile, rule_id))
        .any(|rule_id| failures.contains(&rule_id));
    assert_eq!(target_failed, should_fail, "{profile}: {report}");
}

/// Maps a canonical PDF/A-1 local rule ID to the profile-specific PDF/A-2/3 ID.
pub fn pdfa_profile_local_rule_id(profile: ReferenceProfile, rule_id: &str) -> String {
    let (prefix, suffix) = match profile {
        ReferenceProfile::PdfA2a => (
            "PDFA2A",
            rule_id
                .strip_prefix("PDFA1A-")
                .or_else(|| rule_id.strip_prefix("PDFA1B-")),
        ),
        ReferenceProfile::PdfA2b => (
            "PDFA2B",
            rule_id
                .strip_prefix("PDFA1A-")
                .or_else(|| rule_id.strip_prefix("PDFA1B-")),
        ),
        ReferenceProfile::PdfA2u => (
            "PDFA2U",
            rule_id
                .strip_prefix("PDFA1A-")
                .or_else(|| rule_id.strip_prefix("PDFA1B-")),
        ),
        ReferenceProfile::PdfA3a => (
            "PDFA3A",
            rule_id
                .strip_prefix("PDFA1A-")
                .or_else(|| rule_id.strip_prefix("PDFA1B-")),
        ),
        ReferenceProfile::PdfA3b => (
            "PDFA3B",
            rule_id
                .strip_prefix("PDFA1A-")
                .or_else(|| rule_id.strip_prefix("PDFA1B-")),
        ),
        ReferenceProfile::PdfA3u => (
            "PDFA3U",
            rule_id
                .strip_prefix("PDFA1A-")
                .or_else(|| rule_id.strip_prefix("PDFA1B-")),
        ),
        ReferenceProfile::PdfA1a | ReferenceProfile::PdfA1b | ReferenceProfile::PdfUa1 => {
            return rule_id.to_owned();
        }
    };
    suffix.map_or_else(|| rule_id.to_owned(), |suffix| format!("{prefix}-{suffix}"))
}

/// Writes a generated rule fixture under the ignored-test output directory.
pub fn write_generated_fixture(name: &str, bytes: &[u8]) -> PathBuf {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/generated");
    fs::create_dir_all(&directory).expect("create generated PDF/A fixture directory");
    let path = directory.join(name);
    fs::write(&path, bytes).expect("write generated PDF/A fixture");
    path
}

/// Runs the opt-in pinned veraPDF check for a per-rule profile matrix.
pub fn run_pdfa_rule_differential(
    reference_rule: &str,
    local_rule_ids: &[&str],
    profiles: &[ReferenceProfile],
) {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        eprintln!("VERAPDF_BIN is unset; skipping per-rule PDF/A differential test");
        return;
    };
    let temporary = env::temp_dir().join(format!(
        "page-pdfa-rule-{}-{}",
        std::process::id(),
        reference_rule.replace([':', '.'], "-")
    ));
    fs::create_dir_all(&temporary).expect("create per-rule differential directory");
    for profile in profiles {
        let valid = pdfa_profile_fixture(*profile, canonical_pdfa_fixture(*profile));
        let valid_path = temporary.join(format!("{profile}-valid.pdf"));
        fs::write(&valid_path, valid).expect("write valid per-rule fixture");
        compare_pdfa_rule_case(
            &executable,
            *profile,
            reference_rule,
            local_rule_ids,
            &valid_path,
            false,
        );

        if matches!(profile, ReferenceProfile::PdfA1b) {
            for local_rule_id in local_rule_ids {
                let Some(source) = mutation_fixture(local_rule_id) else {
                    continue;
                };
                let invalid = pdfa_profile_fixture(*profile, &source);
                let invalid_path = temporary.join(format!("{profile}-{local_rule_id}.pdf"));
                fs::write(&invalid_path, invalid).expect("write invalid per-rule fixture");
                compare_pdfa_rule_case(
                    &executable,
                    *profile,
                    reference_rule,
                    local_rule_ids,
                    &invalid_path,
                    true,
                );
                break;
            }
        }
    }
    fs::remove_dir_all(temporary).expect("remove per-rule differential directory");
}

fn compare_pdfa_rule_case(
    executable: &std::ffi::OsStr,
    profile: ReferenceProfile,
    reference_rule: &str,
    local_rule_ids: &[&str],
    path: &Path,
    should_fail: bool,
) {
    let mut config = ReferenceConfig::pinned(executable.to_owned());
    config.profile = profile;
    config.coverage_gap_policy =
        page_validation::differential::CoverageGapPolicy::RejectForCompleteProfile;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF 1.30.2");
    let report = runner.compare_file(path, &SafetyLimits::default());
    assert!(
        !matches!(
            report.classification,
            ComparisonClassification::CoverageGap
                | ComparisonClassification::LocalFalseNegative
                | ComparisonClassification::LocalParserDiscrepancy
                | ComparisonClassification::ReferenceParserDiscrepancy
                | ComparisonClassification::Operational
        ),
        "{report}"
    );
    let reference_failed = report
        .reference_result
        .as_ref()
        .expect("veraPDF result")
        .failed_rule_ids
        .iter()
        .any(|rule| rule.to_string() == reference_rule);
    let local_failed = report.local_report.failures.iter().any(|failure| {
        local_rule_ids
            .iter()
            .map(|rule_id| pdfa_profile_local_rule_id(profile, rule_id))
            .any(|rule_id| rule_id == failure.rule_id)
    });
    assert_eq!(reference_failed, should_fail, "{path:?}: {report}");
    assert_eq!(local_failed, should_fail, "{path:?}: {report}");
}

pub fn metadata_fixture(case: &str) -> Vec<u8> {
    let mut xmp = BASE_XMP.to_owned();
    let mut metadata_dictionary = dictionary! {
        "Type" => "Metadata",
        "Subtype" => "XML",
    };
    let mut include_metadata = true;
    let mut info = complete_info();
    let mut compress_metadata = false;
    let mut encode_xmp_utf16le = false;
    let mut encode_xmp_utf32be = false;

    if case.starts_with("extension_") {
        let replacement = format!("{EXTENSION_SCHEMA_BLOCK}</rdf:RDF>");
        replace(&mut xmp, "</rdf:RDF>", &replacement);
    }
    if case.starts_with("id_") {
        replace(
            &mut xmp,
            "xmlns:pdfaid=\"http://www.aiim.org/pdfa/ns/id/\"",
            "xmlns:pdfaid=\"http://www.aiim.org/pdfa/ns/id/\"\n xmlns:idAlias=\"http://www.aiim.org/pdfa/ns/id/\"",
        );
    }

    match case {
        "baseline_b" => {}
        "first_rdf_package" => {
            replace(
                &mut xmp,
                "<?xpacket begin=\"\"?>",
                "<?xpacket begin=\"\"?><wrapper>",
            );
            replace(
                &mut xmp,
                "</rdf:RDF>",
                "</rdf:RDF><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\" xmlns:pdfaid=\"http://www.aiim.org/pdfa/ns/id/\"><rdf:Description pdfaid:part=\"2\" pdfaid:conformance=\"A\"/></rdf:RDF>",
            );
            replace(
                &mut xmp,
                "<?xpacket end=\"w\"?>",
                "</wrapper><?xpacket end=\"w\"?>",
            );
        }
        "empty_xmpmeta_stops_search" => {
            replace(
                &mut xmp,
                "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">",
                "<wrapper xmlns:x=\"adobe:ns:meta/\"><x:xmpmeta/>",
            );
            replace(&mut xmp, "</x:xmpmeta>", "</wrapper>");
        }
        "ix_changes_ignored" => {
            replace(
                &mut xmp,
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\">",
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n xmlns:iX=\"http://ns.adobe.com/iX/1.0/\">",
            );
            replace(
                &mut xmp,
                "<dc:title>",
                "<iX:changes><rdf:Description><iX:unknown><![CDATA[ignored]]></iX:unknown></rdf:Description></iX:changes><dc:title>",
            );
        }
        "rdf_cdata_literal" => {
            replace(&mut xmp, " pdf:Producer=\"producer\"", "");
            replace(
                &mut xmp,
                "<dc:title>",
                "<pdf:Producer><![CDATA[producer]]></pdf:Producer><dc:title>",
            );
        }
        "rdf_comment_between_properties" => replace(
            &mut xmp,
            "<dc:title>",
            "<!-- comments are RDF child nodes --><dc:title>",
        ),
        "localized_language_normalization" => replace(
            &mut xmp,
            "<rdf:li xml:lang=\"fr\">Titre</rdf:li>\n<rdf:li xml:lang=\"x-default\">Title</rdf:li>",
            "<rdf:li xml:lang=\"fr\">Wrong title</rdf:li>\n<rdf:li xml:lang=\"X_DEFAULT\">Title</rdf:li>",
        ),
        "rdf_nbsp_between_descriptions" => replace(
            &mut xmp,
            "</rdf:Description>\n</rdf:RDF>",
            "</rdf:Description>\u{a0}<rdf:Description/>\n</rdf:RDF>",
        ),
        "deprecated_dc_namespace" => replace(
            &mut xmp,
            "http://purl.org/dc/elements/1.1/",
            "http://purl.org/dc/1.1/",
        ),
        "lang_alt_partial_language" => replace(
            &mut xmp,
            "<dc:title>",
            "<dc:rights><rdf:Alt><rdf:li xml:lang=\"x-default\">Rights</rdf:li><rdf:li>Unqualified</rdf:li></rdf:Alt></dc:rights><dc:title>",
        ),
        "unknown_rdf_property" => replace(
            &mut xmp,
            "<rdf:Description pdfaid:part=",
            "<rdf:Description rdf:unknown=\"value\" pdfaid:part=",
        ),
        "mismatched_rdf_about" => {
            replace(
                &mut xmp,
                "<rdf:Description pdfaid:part=",
                "<rdf:Description rdf:about=\"one\" pdfaid:part=",
            );
            replace(
                &mut xmp,
                "</rdf:RDF>",
                "<rdf:Description rdf:about=\"two\"/></rdf:RDF>",
            );
        }
        "rdf_resource_and_value" => replace(
            &mut xmp,
            "<dc:title>",
            "<xmp:Nickname rdf:resource=\"urn:value\" rdf:value=\"value\"/><dc:title>",
        ),
        "duplicate_producer" => replace(
            &mut xmp,
            "</rdf:RDF>",
            "<rdf:Description pdf:Producer=\"second\"/></rdf:RDF>",
        ),
        "predefined_structured_field" => {
            replace(
                &mut xmp,
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\">",
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n xmlns:exif=\"http://ns.adobe.com/exif/1.0/\">",
            );
            replace(
                &mut xmp,
                "<dc:title>",
                "<exif:Flash rdf:parseType=\"Resource\"><exif:Fired>True</exif:Fired></exif:Flash><dc:title>",
            );
        }
        "predefined_structured_attribute_fields" => {
            replace(
                &mut xmp,
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\">",
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n xmlns:exif=\"http://ns.adobe.com/exif/1.0/\">",
            );
            replace(
                &mut xmp,
                "<dc:title>",
                "<exif:Flash><rdf:Description exif:Fired=\"True\" exif:Mode=\"1\"/></exif:Flash><dc:title>",
            );
        }
        "info_empty_property_value" => {
            replace(&mut xmp, " pdf:Keywords=\"rust,pdf\"", "");
            replace(
                &mut xmp,
                "<dc:title>",
                "<pdf:Keywords rdf:value=\"rust,pdf\"/><dc:title>",
            );
        }
        "keywords_qualified_parse_type" => {
            replace(
                &mut xmp,
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\">",
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\" xmlns:q=\"urn:qualifier\">",
            );
            replace(&mut xmp, " pdf:Keywords=\"rust,pdf\"", "");
            replace(
                &mut xmp,
                "<dc:title>",
                "<pdf:Keywords rdf:parseType=\"Resource\"><rdf:value>rust,pdf</rdf:value><q:kind>qualified</q:kind></pdf:Keywords><dc:title>",
            );
        }
        "title_qualified_expanded" => {
            replace(
                &mut xmp,
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\">",
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\" xmlns:q=\"urn:qualifier\">",
            );
            replace(
                &mut xmp,
                "<dc:title><rdf:Alt><rdf:li xml:lang=\"fr\">Titre</rdf:li>\n<rdf:li xml:lang=\"x-default\">Title</rdf:li></rdf:Alt></dc:title>",
                "<dc:title><rdf:Description><rdf:value><rdf:Alt><rdf:li xml:lang=\"fr\">Titre</rdf:li>\n<rdf:li xml:lang=\"x-default\">Title</rdf:li></rdf:Alt></rdf:value><q:kind>qualified</q:kind></rdf:Description></dc:title>",
            );
        }
        "keywords_default_element" => {
            replace(&mut xmp, " pdf:Keywords=\"rust,pdf\"", "");
            replace(
                &mut xmp,
                "<dc:title>",
                "<Keywords xmlns=\"http://ns.adobe.com/pdf/1.3/\">rust,pdf</Keywords><dc:title>",
            );
        }
        "reordered_top_level_properties" => {
            replace(&mut xmp, " pdf:Producer=\"producer\"", "");
            replace(
                &mut xmp,
                "</rdf:Alt></dc:description>",
                "</rdf:Alt></dc:description><pdf:Producer>producer</pdf:Producer>",
            );
        }
        "xmp_latin1_recovery" => {
            info.set(
                "Producer",
                Object::String(vec![b'c', b'a', b'f', 0xE9], StringFormat::Literal),
            );
            replace(
                &mut xmp,
                "pdf:Producer=\"producer\"",
                "pdf:Producer=\"caf~\"",
            );
            let marker = xmp.iter().position(|byte| *byte == b'~').expect("marker");
            *xmp.get_mut(marker).expect("Latin-1 recovery marker") = 0xE9;
        }
        "xmp_ascii_control_reference_recovery" => {
            info.set("Producer", Object::string_literal("a b"));
            replace(
                &mut xmp,
                "pdf:Producer=\"producer\"",
                "pdf:Producer=\"a&#x1;b\"",
            );
        }
        "xmp_del_reference_preserved" => {
            info.set("Producer", Object::string_literal("a b"));
            replace(
                &mut xmp,
                "pdf:Producer=\"producer\"",
                "pdf:Producer=\"a&#x7F;b\"",
            );
        }
        "xmp_utf16le_without_bom" => encode_xmp_utf16le = true,
        "xmp_utf32be_without_bom" => encode_xmp_utf32be = true,
        "rdf_parse_type_literal" => replace(
            &mut xmp,
            "<dc:title>",
            "<xmp:Nickname rdf:parseType=\"Literal\">nickname</xmp:Nickname><dc:title>",
        ),
        "no_rdf_package" => {
            let begin = xmp
                .windows(b"<rdf:RDF".len())
                .position(|window| window == b"<rdf:RDF")
                .expect("RDF start");
            let end = xmp
                .windows(b"</rdf:RDF>".len())
                .position(|window| window == b"</rdf:RDF>")
                .expect("RDF end")
                + b"</rdf:RDF>".len();
            xmp.splice(begin..end, b"<not-xmp/>".iter().copied());
        }
        "gps_coordinate_invalid" => {
            replace(
                &mut xmp,
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\">",
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n xmlns:exif=\"http://ns.adobe.com/exif/1.0/\">",
            );
            replace(
                &mut xmp,
                "pdf:Keywords=\"rust,pdf\"",
                "pdf:Keywords=\"rust,pdf\" exif:GPSLatitude=\"invalid\"",
            );
        }
        "predefined_unknown_property" => replace(
            &mut xmp,
            "pdf:Keywords=\"rust,pdf\"",
            "pdf:Keywords=\"rust,pdf\" pdf:Unknown=\"invalid\"",
        ),
        "id_alias_declaration_only" => {}
        "id_corr_prefix" => replace(
            &mut xmp,
            "<dc:title>",
            "<idAlias:corr>1</idAlias:corr><dc:title>",
        ),
        "id_part_alias" => replace(&mut xmp, "pdfaid:part=\"1\"", "idAlias:part=\"1\""),
        "id_part_plus_one" => replace(&mut xmp, "pdfaid:part=\"1\"", "pdfaid:part=\"+1\""),
        "id_part_leading_zero" => replace(&mut xmp, "pdfaid:part=\"1\"", "pdfaid:part=\"01\""),
        "id_part_unicode_digit" => replace(&mut xmp, "pdfaid:part=\"1\"", "pdfaid:part=\"١\""),
        "id_conformance_alias" => replace(
            &mut xmp,
            "pdfaid:conformance=\"B\"",
            "idAlias:conformance=\"B\"",
        ),
        "id_amd_canonical" => replace(
            &mut xmp,
            "pdfaid:part=\"1\"",
            "pdfaid:amd=\"1:2005\" pdfaid:part=\"1\"",
        ),
        "id_amd_alias" => replace(
            &mut xmp,
            "pdfaid:part=\"1\"",
            "idAlias:amd=\"1:2005\" pdfaid:part=\"1\"",
        ),
        "id_part_default_element" => {
            replace(&mut xmp, " pdfaid:part=\"1\"", "");
            replace(
                &mut xmp,
                "<dc:title>",
                "<part xmlns=\"http://www.aiim.org/pdfa/ns/id/\">1</part><dc:title>",
            );
        }
        "id_conformance_default_element" => {
            replace(&mut xmp, " pdfaid:conformance=\"B\"", "");
            replace(
                &mut xmp,
                "<dc:title>",
                "<conformance xmlns=\"http://www.aiim.org/pdfa/ns/id/\">B</conformance><dc:title>",
            );
        }
        "id_amd_default_element" => replace(
            &mut xmp,
            "<dc:title>",
            "<amd xmlns=\"http://www.aiim.org/pdfa/ns/id/\">1:2005</amd><dc:title>",
        ),
        "extension_container_attribute" => replace(
            &mut xmp,
            EXTENSION_SCHEMA_BLOCK,
            "<rdf:Description xmlns:pdfaExtension=\"http://www.aiim.org/pdfa/ns/extension/\" pdfaExtension:schemas=\"invalid\"/>",
        ),
        "extension_valid" => {}
        "extension_custom_simple_type" => {
            replace(
                &mut xmp,
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\">",
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n xmlns:ex=\"http://example.com/ns/\">",
            );
            replace(
                &mut xmp,
                "<pdfaProperty:name>example</pdfaProperty:name>\n<pdfaProperty:valueType>Text</pdfaProperty:valueType>",
                "<pdfaProperty:name>simple</pdfaProperty:name>\n<pdfaProperty:valueType>CustomType</pdfaProperty:valueType>",
            );
            replace(
                &mut xmp,
                "<pdfaType:field><rdf:Seq>\n<rdf:li rdf:parseType=\"Resource\">\n<pdfaField:name>member</pdfaField:name>\n<pdfaField:valueType>Text</pdfaField:valueType>\n<pdfaField:description>Example member</pdfaField:description>\n</rdf:li>\n</rdf:Seq></pdfaType:field>\n",
                "",
            );
            replace(
                &mut xmp,
                "</rdf:RDF>",
                "<rdf:Description><ex:simple>value</ex:simple></rdf:Description></rdf:RDF>",
            );
        }
        "extension_unknown_declared_property_used" => {
            replace(
                &mut xmp,
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\">",
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n xmlns:ex=\"http://example.com/ns/\">",
            );
            replace(
                &mut xmp,
                "<pdfaProperty:valueType>Text</pdfaProperty:valueType>",
                "<pdfaProperty:valueType>UnknownType</pdfaProperty:valueType>",
            );
            replace(
                &mut xmp,
                "</rdf:RDF>",
                "<rdf:Description><ex:example>value</ex:example></rdf:Description></rdf:RDF>",
            );
        }
        "extension_namespace_whitespace" => {
            replace(
                &mut xmp,
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\">",
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n xmlns:ex=\"http://example.com/ns/\">",
            );
            replace(
                &mut xmp,
                "<pdfaSchema:namespaceURI>http://example.com/ns/</pdfaSchema:namespaceURI>",
                "<pdfaSchema:namespaceURI> http://example.com/ns/ </pdfaSchema:namespaceURI>",
            );
            replace(
                &mut xmp,
                "</rdf:RDF>",
                "<rdf:Description><ex:example>value</ex:example></rdf:Description></rdf:RDF>",
            );
        }
        "extension_duplicate_namespace_replaces" => {
            replace(
                &mut xmp,
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\">",
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n xmlns:ex=\"http://example.com/ns/\">",
            );
            replace(
                &mut xmp,
                "</rdf:Bag></pdfaExtension:schemas>",
                "<rdf:li rdf:parseType=\"Resource\">\n<pdfaSchema:schema>Replacement schema</pdfaSchema:schema>\n<pdfaSchema:namespaceURI>http://example.com/ns/</pdfaSchema:namespaceURI>\n<pdfaSchema:prefix>ex</pdfaSchema:prefix>\n<pdfaSchema:property><rdf:Seq><rdf:li rdf:parseType=\"Resource\">\n<pdfaProperty:name>replacement</pdfaProperty:name>\n<pdfaProperty:valueType>Text</pdfaProperty:valueType>\n<pdfaProperty:category>external</pdfaProperty:category>\n<pdfaProperty:description>Replacement property</pdfaProperty:description>\n</rdf:li></rdf:Seq></pdfaSchema:property>\n</rdf:li></rdf:Bag></pdfaExtension:schemas>",
            );
            replace(
                &mut xmp,
                "</rdf:RDF>",
                "<rdf:Description><ex:example>value</ex:example></rdf:Description></rdf:RDF>",
            );
        }
        "extension_duplicate_property_keeps_first" => {
            replace(
                &mut xmp,
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\">",
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n xmlns:ex=\"http://example.com/ns/\">",
            );
            replace(
                &mut xmp,
                "</rdf:li>\n</rdf:Seq></pdfaSchema:property>",
                "</rdf:li>\n<rdf:li rdf:parseType=\"Resource\">\n<pdfaProperty:name>example</pdfaProperty:name>\n<pdfaProperty:valueType>Integer</pdfaProperty:valueType>\n<pdfaProperty:category>external</pdfaProperty:category>\n<pdfaProperty:description>Replacement property</pdfaProperty:description>\n</rdf:li>\n</rdf:Seq></pdfaSchema:property>",
            );
            replace(
                &mut xmp,
                "</rdf:RDF>",
                "<rdf:Description><ex:example>value</ex:example></rdf:Description></rdf:RDF>",
            );
        }
        "extension_rational_value_type" => {
            replace(
                &mut xmp,
                "<pdfaProperty:valueType>Text</pdfaProperty:valueType>",
                "<pdfaProperty:valueType>rational</pdfaProperty:valueType>",
            );
            replace(
                &mut xmp,
                "<pdfaField:valueType>Text</pdfaField:valueType>",
                "<pdfaField:valueType>GPSCoordinate</pdfaField:valueType>",
            );
        }
        "extension_xpath_invalid" => {
            replace(
                &mut xmp,
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\">",
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n xmlns:ex=\"http://example.com/ns/\">",
            );
            replace(
                &mut xmp,
                "<pdfaProperty:valueType>Text</pdfaProperty:valueType>",
                "<pdfaProperty:valueType>XPath</pdfaProperty:valueType>",
            );
            replace(
                &mut xmp,
                "</rdf:RDF>",
                "<rdf:Description><ex:example>/*[</ex:example></rdf:Description></rdf:RDF>",
            );
        }
        "extension_undefined_field" => replace(
            &mut xmp,
            "<pdfaSchema:schema>Example schema</pdfaSchema:schema>",
            "<pdfaSchema:schema>Example schema</pdfaSchema:schema><pdfaSchema:unknown>bad</pdfaSchema:unknown>",
        ),
        "extension_custom_value_invalid" => {
            replace(
                &mut xmp,
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\">",
                " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n xmlns:ex=\"http://example.com/ns/\" xmlns:extype=\"http://example.com/type/\">",
            );
            replace(
                &mut xmp,
                "<pdfaProperty:name>example</pdfaProperty:name>\n<pdfaProperty:valueType>Text</pdfaProperty:valueType>",
                "<pdfaProperty:name>custom</pdfaProperty:name>\n<pdfaProperty:valueType>CustomType</pdfaProperty:valueType>",
            );
            replace(
                &mut xmp,
                "</rdf:RDF>",
                "<rdf:Description><ex:custom rdf:parseType=\"Resource\"><extype:member rdf:parseType=\"Resource\"/></ex:custom></rdf:Description></rdf:RDF>",
            );
        }
        "extension_container_prefix" => {
            replace(
                &mut xmp,
                "<pdfaExtension:schemas>",
                "<extensionAlias:schemas>",
            );
            replace(
                &mut xmp,
                "</pdfaExtension:schemas>",
                "</extensionAlias:schemas>",
            );
        }
        "extension_container_seq" => {
            replace(&mut xmp, "<rdf:Bag>", "<rdf:Seq>");
            replace(&mut xmp, "</rdf:Bag>", "</rdf:Seq>");
        }
        "extension_schema_name_prefix" => {
            replace(&mut xmp, "<pdfaSchema:schema>", "<schemaAlias:schema>");
            replace(&mut xmp, "</pdfaSchema:schema>", "</schemaAlias:schema>");
        }
        "extension_schema_namespace_prefix" => {
            replace(
                &mut xmp,
                "<pdfaSchema:namespaceURI>http://example.com/ns/</pdfaSchema:namespaceURI>",
                "<schemaAlias:namespaceURI>http://example.com/ns/</schemaAlias:namespaceURI>",
            );
        }
        "extension_schema_prefix_prefix" => {
            replace(
                &mut xmp,
                "<pdfaSchema:prefix>ex</pdfaSchema:prefix>",
                "<schemaAlias:prefix>ex</schemaAlias:prefix>",
            );
        }
        "extension_property_bag" => {
            replace(
                &mut xmp,
                "<pdfaSchema:property><rdf:Seq>",
                "<pdfaSchema:property><rdf:Bag>",
            );
            replace(
                &mut xmp,
                "</rdf:Seq></pdfaSchema:property>",
                "</rdf:Bag></pdfaSchema:property>",
            );
        }
        "extension_value_type_bag" => {
            replace(
                &mut xmp,
                "<pdfaSchema:valueType><rdf:Seq>",
                "<pdfaSchema:valueType><rdf:Bag>",
            );
            replace(
                &mut xmp,
                "</rdf:Seq></pdfaSchema:valueType>",
                "</rdf:Bag></pdfaSchema:valueType>",
            );
        }
        "extension_property_name_prefix" => {
            replace(
                &mut xmp,
                "<pdfaProperty:name>example</pdfaProperty:name>",
                "<propertyAlias:name>example</propertyAlias:name>",
            );
        }
        "extension_property_value_type_prefix" => {
            replace(
                &mut xmp,
                "<pdfaProperty:valueType>Text</pdfaProperty:valueType>",
                "<propertyAlias:valueType>Text</propertyAlias:valueType>",
            );
        }
        "extension_property_unknown_value_type" => replace(
            &mut xmp,
            "<pdfaProperty:valueType>Text</pdfaProperty:valueType>",
            "<pdfaProperty:valueType>UnknownType</pdfaProperty:valueType>",
        ),
        "extension_property_category_prefix" => {
            replace(
                &mut xmp,
                "<pdfaProperty:category>external</pdfaProperty:category>",
                "<propertyAlias:category>external</propertyAlias:category>",
            );
        }
        "extension_property_bad_category" => replace(
            &mut xmp,
            "<pdfaProperty:category>external</pdfaProperty:category>",
            "<pdfaProperty:category>invalid</pdfaProperty:category>",
        ),
        "extension_property_description_prefix" => {
            replace(
                &mut xmp,
                "<pdfaProperty:description>Example property</pdfaProperty:description>",
                "<propertyAlias:description>Example property</propertyAlias:description>",
            );
        }
        "extension_value_type_name_prefix" => {
            replace(
                &mut xmp,
                "<pdfaType:type>CustomType</pdfaType:type>",
                "<typeAlias:type>CustomType</typeAlias:type>",
            );
        }
        "extension_value_type_namespace_prefix" => {
            replace(
                &mut xmp,
                "<pdfaType:namespaceURI>http://example.com/type/</pdfaType:namespaceURI>",
                "<typeAlias:namespaceURI>http://example.com/type/</typeAlias:namespaceURI>",
            );
        }
        "extension_value_type_prefix_prefix" => {
            replace(
                &mut xmp,
                "<pdfaType:prefix>extype</pdfaType:prefix>",
                "<typeAlias:prefix>extype</typeAlias:prefix>",
            );
        }
        "extension_value_type_description_prefix" => {
            replace(
                &mut xmp,
                "<pdfaType:description>Example type</pdfaType:description>",
                "<typeAlias:description>Example type</typeAlias:description>",
            );
        }
        "extension_field_bag" => {
            replace(
                &mut xmp,
                "<pdfaType:field><rdf:Seq>",
                "<pdfaType:field><rdf:Bag>",
            );
            replace(
                &mut xmp,
                "</rdf:Seq></pdfaType:field>",
                "</rdf:Bag></pdfaType:field>",
            );
        }
        "extension_field_name_prefix" => {
            replace(
                &mut xmp,
                "<pdfaField:name>member</pdfaField:name>",
                "<fieldAlias:name>member</fieldAlias:name>",
            );
        }
        "extension_field_value_type_prefix" => {
            replace(
                &mut xmp,
                "<pdfaField:valueType>Text</pdfaField:valueType>",
                "<fieldAlias:valueType>Text</fieldAlias:valueType>",
            );
        }
        "extension_field_unknown_value_type" => replace(
            &mut xmp,
            "<pdfaField:valueType>Text</pdfaField:valueType>",
            "<pdfaField:valueType>UnknownType</pdfaField:valueType>",
        ),
        "extension_field_description_prefix" => {
            replace(
                &mut xmp,
                "<pdfaField:description>Example member</pdfaField:description>",
                "<fieldAlias:description>Example member</fieldAlias:description>",
            );
        }
        "accepted_a" => replace(
            &mut xmp,
            "pdfaid:conformance=\"B\"",
            "pdfaid:conformance=\"A\"",
        ),
        "missing_metadata" => include_metadata = false,
        "missing_type" => {
            metadata_dictionary.remove(b"Type");
        }
        "missing_subtype" => {
            metadata_dictionary.remove(b"Subtype");
        }
        "metadata_filter" => compress_metadata = true,
        "packet_bytes_double" => replace(
            &mut xmp,
            "<?xpacket begin=\"\"?>",
            "<?xpacket begin=\"\" bytes=\"123\"?>",
        ),
        "packet_bytes_single" => replace(
            &mut xmp,
            "<?xpacket begin=\"\"?>",
            "<?xpacket begin=\"\" bytes='123'?>",
        ),
        "packet_bytes_spaced" => replace(
            &mut xmp,
            "<?xpacket begin=\"\"?>",
            "<?xpacket begin=\"\" bytes \t= \t\"123\"?>",
        ),
        "packet_encoding" => replace(
            &mut xmp,
            "<?xpacket begin=\"\"?>",
            "<?xpacket begin=\"\" encoding=\"UTF-8\"?>",
        ),
        "packet_bytes_and_encoding" => replace(
            &mut xmp,
            "<?xpacket begin=\"\"?>",
            "<?xpacket begin=\"\" bytes=\"123\" encoding='UTF-8'?>",
        ),
        "packet_uppercase_bytes" => replace(
            &mut xmp,
            "<?xpacket begin=\"\"?>",
            "<?xpacket begin=\"\" Bytes=\"123\"?>",
        ),
        "packet_unquoted_bytes" => replace(
            &mut xmp,
            "<?xpacket begin=\"\"?>",
            "<?xpacket begin=\"\" bytes=123?>",
        ),
        "packet_substring_bytes" => replace(
            &mut xmp,
            "<?xpacket begin=\"\"?>",
            "<?xpacket begin=\"\" mybytes=\"123\"?>",
        ),
        "packet_body_bytes" => replace(
            &mut xmp,
            "<rdf:Description pdfaid:part=",
            "<rdf:Description bytes=\"123\" pdfaid:part=",
        ),
        "packet_end_bytes" => replace(
            &mut xmp,
            "<?xpacket end=\"w\"?>",
            "<?xpacket end=\"w\" bytes=\"123\"?>",
        ),
        "packet_first_forbidden_then_clean" => replace(
            &mut xmp,
            "<?xpacket begin=\"\"?>",
            "<?xpacket begin=\"\" bytes=\"123\"?><?xpacket begin=\"\"?>",
        ),
        "packet_clean_then_forbidden" => replace(
            &mut xmp,
            "<?xpacket begin=\"\"?>",
            "<?xpacket begin=\"\"?><?xpacket begin=\"\" bytes=\"123\"?>",
        ),
        "malformed_xmp" => xmp = b"<rdf:RDF>".to_vec(),
        "missing_identification" => {
            replace(&mut xmp, " pdfaid:part=\"1\" pdfaid:conformance=\"B\"", "");
        }
        "missing_conformance" => {
            replace(&mut xmp, " pdfaid:conformance=\"B\"", "");
        }
        "wrong_part" => replace(&mut xmp, "pdfaid:part=\"1\"", "pdfaid:part=\"2\""),
        "wrong_part_four" => replace(&mut xmp, "pdfaid:part=\"1\"", "pdfaid:part=\"4\""),
        "lowercase_conformance" => replace(
            &mut xmp,
            "pdfaid:conformance=\"B\"",
            "pdfaid:conformance=\"b\"",
        ),
        "duplicate_identification" => replace(
            &mut xmp,
            "</rdf:RDF>",
            "<rdf:Description pdfaid:part=\"2\" pdfaid:conformance=\"A\"/></rdf:RDF>",
        ),
        "title_mismatch" => info.set("Title", Object::string_literal("different")),
        "title_whitespace_equivalent" => {
            info.set("Title", Object::string_literal(" Title "));
            replace(
                &mut xmp,
                "<rdf:li xml:lang=\"x-default\">Title</rdf:li>",
                "<rdf:li xml:lang=\"x-default\"> Title </rdf:li>",
            );
        }
        "title_pdfdoc_equivalent" => {
            info.set(
                "Title",
                Object::String(
                    vec![b't', b'e', b'x', b't', 0x8B],
                    lopdf::StringFormat::Literal,
                ),
            );
            replace(
                &mut xmp,
                "<rdf:li xml:lang=\"x-default\">Title</rdf:li>",
                "<rdf:li xml:lang=\"x-default\">text‰</rdf:li>",
            );
        }
        "title_utf16_equivalent" => {
            info.set(
                "Title",
                Object::String(
                    vec![0xFE, 0xFF, 0x00, b'C', 0x00, b'a', 0x00, b'f', 0x00, 0xE9],
                    lopdf::StringFormat::Literal,
                ),
            );
            replace(
                &mut xmp,
                "<rdf:li xml:lang=\"x-default\">Title</rdf:li>",
                "<rdf:li xml:lang=\"x-default\">Café</rdf:li>",
            );
        }
        "title_utf8_bom_equivalent" => {
            info.set(
                "Title",
                Object::String(
                    vec![0xEF, 0xBB, 0xBF, b'C', b'a', b'f', 0xC3, 0xA9],
                    lopdf::StringFormat::Literal,
                ),
            );
            replace(
                &mut xmp,
                "<rdf:li xml:lang=\"x-default\">Title</rdf:li>",
                "<rdf:li xml:lang=\"x-default\">Café</rdf:li>",
            );
        }
        "title_utf16_odd_equivalent" => {
            info.set(
                "Title",
                Object::String(
                    vec![0xFE, 0xFF, 0x00, b'A', 0x00],
                    lopdf::StringFormat::Literal,
                ),
            );
            replace(
                &mut xmp,
                "<rdf:li xml:lang=\"x-default\">Title</rdf:li>",
                "<rdf:li xml:lang=\"x-default\">A�</rdf:li>",
            );
        }
        "title_fallback_first_alternative" => {
            info.set("Title", Object::string_literal("Titre"));
            replace(
                &mut xmp,
                "<rdf:li xml:lang=\"x-default\">Title</rdf:li>",
                "<rdf:li xml:lang=\"en\">Title</rdf:li>",
            );
        }
        "title_fallback_generic_x" => replace(
            &mut xmp,
            "<rdf:li xml:lang=\"x-default\">Title</rdf:li>",
            "<rdf:li xml:lang=\"x-private\">Title</rdf:li>",
        ),
        "author_mismatch" => info.set("Author", Object::string_literal("different")),
        "author_ordered_alt" => {
            replace(&mut xmp, "<dc:creator><rdf:Seq>", "<dc:creator><rdf:Alt>");
            replace(
                &mut xmp,
                "</rdf:Seq></dc:creator>",
                "</rdf:Alt></dc:creator>",
            );
        }
        "subject_mismatch" => info.set("Subject", Object::string_literal("different")),
        "keywords_mismatch" => info.set("Keywords", Object::string_literal("different")),
        "keywords_xmp_whitespace" => replace(
            &mut xmp,
            "pdf:Keywords=\"rust,pdf\"",
            "pdf:Keywords=\" rust,pdf \"",
        ),
        "creator_mismatch" => info.set("Creator", Object::string_literal("different")),
        "producer_mismatch" => info.set("Producer", Object::string_literal("different")),
        "producer_trailing_nul" => info.set(
            "Producer",
            Object::String(b"producer\0".to_vec(), lopdf::StringFormat::Literal),
        ),
        "producer_two_trailing_nuls" => info.set(
            "Producer",
            Object::String(b"producer\0\0".to_vec(), lopdf::StringFormat::Literal),
        ),
        "creation_date_equivalent_offset" => replace(
            &mut xmp,
            "2026-07-27T12:30:45+02:00",
            "2026-07-27T10:30:45Z",
        ),
        "creation_date_pdf_z00_equivalent" => {
            info.set(
                "CreationDate",
                Object::string_literal("D:20260727103045Z00"),
            );
            replace_first(
                &mut xmp,
                "2026-07-27T12:30:45+02:00",
                "2026-07-27T10:30:45Z",
            );
        }
        "creation_date_pdf_z00_00_equivalent" => {
            info.set(
                "CreationDate",
                Object::string_literal("D:20260727103045Z00'00'"),
            );
            replace_first(
                &mut xmp,
                "2026-07-27T12:30:45+02:00",
                "2026-07-27T10:30:45Z",
            );
        }
        "creation_date_pdf_unicode_digits_equivalent" => {
            let mut encoded = vec![0xFE, 0xFF];
            encoded.extend(
                "D:٢٠٢٦٠٧٢٧١٢٣٠٤٥+٠٢'٠٠'"
                    .encode_utf16()
                    .flat_map(u16::to_be_bytes),
            );
            info.set(
                "CreationDate",
                Object::String(encoded, StringFormat::Literal),
            );
        }
        "creation_date_historic_same_lexical" => {
            info.set("CreationDate", Object::string_literal("D:15000228000000Z"));
            replace_first(
                &mut xmp,
                "2026-07-27T12:30:45+02:00",
                "1500-02-28T00:00:00Z",
            );
        }
        "creation_date_historic_shifted_equivalent" => {
            info.set("CreationDate", Object::string_literal("D:15000228000000Z"));
            replace_first(
                &mut xmp,
                "2026-07-27T12:30:45+02:00",
                "1500-03-09T00:00:00Z",
            );
        }
        "creation_date_long_fraction_equivalent" => {
            info.set("CreationDate", Object::string_literal("D:20260727103045Z"));
            replace_first(
                &mut xmp,
                "2026-07-27T12:30:45+02:00",
                "2026-07-27T10:30:45.0000000001Z",
            );
        }
        "creation_date_mismatch" => {
            replace_first(
                &mut xmp,
                "2026-07-27T12:30:45+02:00",
                "2026-07-28T12:30:45+02:00",
            );
        }
        "creation_date_invalid" => {
            replace_first(&mut xmp, "2026-07-27T12:30:45+02:00", "not-a-date");
        }
        "creation_date_reduced_mismatch" => {
            replace_first(&mut xmp, "2026-07-27T12:30:45+02:00", "2026-07");
        }
        "creation_date_submillisecond_equivalent" => {
            replace_first(
                &mut xmp,
                "2026-07-27T12:30:45+02:00",
                "2026-07-27T12:30:45.000000001+02:00",
            );
        }
        "mod_date_mismatch" => {
            replace_last(
                &mut xmp,
                "2026-07-27T12:30:45+02:00",
                "2026-07-28T12:30:45+02:00",
            );
        }
        "author_multiple" => replace(
            &mut xmp,
            "<rdf:li>Author</rdf:li>",
            "<rdf:li>Author</rdf:li><rdf:li>Second</rdf:li>",
        ),
        _ => panic!("unknown metadata fixture case {case}"),
    }

    if encode_xmp_utf16le || encode_xmp_utf32be {
        let xml = String::from_utf8(xmp).expect("fixture XMP is UTF-8");
        xmp = if encode_xmp_utf16le {
            xml.encode_utf16().flat_map(u16::to_le_bytes).collect()
        } else {
            xml.chars()
                .flat_map(|character| u32::from(character).to_be_bytes())
                .collect()
        };
    }

    let mut document = pdf_document();
    let pages_id = document.new_object_id();
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });
    wrap_pages(&mut document, pages_id, page_id);
    let mut catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    };
    if include_metadata {
        let mut stream = Stream::new(metadata_dictionary, xmp);
        if compress_metadata {
            stream.compress().expect("compress test metadata");
        }
        let metadata_id = document.add_object(stream);
        catalog.set("Metadata", metadata_id);
    }
    let output_intents = single_intent(&mut document, None, Some("GTS_PDFA1"));
    catalog.set("OutputIntents", output_intents.expect("output intent"));
    let catalog_id = document.add_object(catalog);
    let info_id = document.add_object(info);
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).expect("save metadata fixture");
    bytes
}

pub fn pdfua1_rule_5_1_fixture(case: &str) -> Vec<u8> {
    let xmp: &[u8] = match case {
        "identification_present" => {
            br#"<?xpacket begin=""?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:pdfuaid="http://www.aiim.org/pdfua/ns/id/" xmlns:dc="http://purl.org/dc/elements/1.1/"><rdf:Description pdfuaid:part="1"><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Document title</rdf:li></rdf:Alt></dc:title></rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end="w"?>"#
        }
        "identification_missing" => {
            br#"<?xpacket begin=""?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:dc="http://purl.org/dc/elements/1.1/"><rdf:Description><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Document title</rdf:li></rdf:Alt></dc:title></rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end="w"?>"#
        }
        _ => panic!("unknown PDF/UA-1 rule 5-1 fixture case {case}"),
    };

    let mut document = pdf_document();
    let pages_id = document.new_object_id();
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });
    wrap_pages(&mut document, pages_id, page_id);
    let metadata_id = document.add_object(Stream::new(
        dictionary! {
            "Type" => "Metadata",
            "Subtype" => "XML",
        },
        xmp.to_vec(),
    ));
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "MarkInfo" => dictionary! { "Marked" => true },
        "ViewerPreferences" => dictionary! { "DisplayDocTitle" => true },
        "StructTreeRoot" => Dictionary::new(),
    });
    document.trailer.set("Root", catalog_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 5-1 fixture");
    bytes
}

pub fn pdfua1_rule_5_2_fixture(case: &str) -> Vec<u8> {
    let xmp: &[u8] = match case {
        "part_one" => {
            br#"<?xpacket begin=""?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:pdfuaid="http://www.aiim.org/pdfua/ns/id/" xmlns:dc="http://purl.org/dc/elements/1.1/"><rdf:Description pdfuaid:part="1"><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Document title</rdf:li></rdf:Alt></dc:title></rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end="w"?>"#
        }
        "part_two" => {
            br#"<?xpacket begin=""?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:pdfuaid="http://www.aiim.org/pdfua/ns/id/" xmlns:dc="http://purl.org/dc/elements/1.1/"><rdf:Description pdfuaid:part="2"><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Document title</rdf:li></rdf:Alt></dc:title></rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end="w"?>"#
        }
        _ => panic!("unknown PDF/UA-1 rule 5-2 fixture case {case}"),
    };

    let mut document = pdf_document();
    let pages_id = document.new_object_id();
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });
    wrap_pages(&mut document, pages_id, page_id);
    let metadata_id = document.add_object(Stream::new(
        dictionary! {
            "Type" => "Metadata",
            "Subtype" => "XML",
        },
        xmp.to_vec(),
    ));
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "MarkInfo" => dictionary! { "Marked" => true },
        "ViewerPreferences" => dictionary! { "DisplayDocTitle" => true },
        "StructTreeRoot" => Dictionary::new(),
    });
    document.trailer.set("Root", catalog_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 5-2 fixture");
    bytes
}

pub fn pdfua1_rule_5_3_fixture(case: &str) -> Vec<u8> {
    let xmp: &[u8] = match case {
        "canonical_prefix" => {
            br#"<?xpacket begin=""?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:pdfuaid="http://www.aiim.org/pdfua/ns/id/" xmlns:dc="http://purl.org/dc/elements/1.1/"><rdf:Description pdfuaid:part="1"><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Document title</rdf:li></rdf:Alt></dc:title></rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end="w"?>"#
        }
        "wrong_prefix" => {
            br#"<?xpacket begin=""?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:wrong="http://www.aiim.org/pdfua/ns/id/" xmlns:dc="http://purl.org/dc/elements/1.1/"><rdf:Description wrong:part="1"><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Document title</rdf:li></rdf:Alt></dc:title></rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end="w"?>"#
        }
        _ => panic!("unknown PDF/UA-1 rule 5-3 fixture case {case}"),
    };

    let mut document = pdf_document();
    let pages_id = document.new_object_id();
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });
    wrap_pages(&mut document, pages_id, page_id);
    let metadata_id = document.add_object(Stream::new(
        dictionary! {
            "Type" => "Metadata",
            "Subtype" => "XML",
        },
        xmp.to_vec(),
    ));
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "MarkInfo" => dictionary! { "Marked" => true },
        "ViewerPreferences" => dictionary! { "DisplayDocTitle" => true },
        "StructTreeRoot" => Dictionary::new(),
    });
    document.trailer.set("Root", catalog_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 5-3 fixture");
    bytes
}

pub fn pdfua1_rule_5_4_fixture(case: &str) -> Vec<u8> {
    let xmp: &[u8] = match case {
        "canonical_prefix" => {
            br#"<?xpacket begin=""?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:pdfuaid="http://www.aiim.org/pdfua/ns/id/" xmlns:dc="http://purl.org/dc/elements/1.1/"><rdf:Description pdfuaid:part="1" pdfuaid:amd="1:2014"><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Document title</rdf:li></rdf:Alt></dc:title></rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end="w"?>"#
        }
        "wrong_prefix" => {
            br#"<?xpacket begin=""?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:pdfuaid="http://www.aiim.org/pdfua/ns/id/" xmlns:wrong="http://www.aiim.org/pdfua/ns/id/" xmlns:dc="http://purl.org/dc/elements/1.1/"><rdf:Description pdfuaid:part="1" wrong:amd="1:2014"><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Document title</rdf:li></rdf:Alt></dc:title></rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end="w"?>"#
        }
        _ => panic!("unknown PDF/UA-1 rule 5-4 fixture case {case}"),
    };

    let mut document = pdf_document();
    let pages_id = document.new_object_id();
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });
    wrap_pages(&mut document, pages_id, page_id);
    let metadata_id = document.add_object(Stream::new(
        dictionary! {
            "Type" => "Metadata",
            "Subtype" => "XML",
        },
        xmp.to_vec(),
    ));
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "MarkInfo" => dictionary! { "Marked" => true },
        "ViewerPreferences" => dictionary! { "DisplayDocTitle" => true },
        "StructTreeRoot" => Dictionary::new(),
    });
    document.trailer.set("Root", catalog_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 5-4 fixture");
    bytes
}

pub fn pdfua1_rule_5_5_fixture(case: &str) -> Vec<u8> {
    let xmp: &[u8] = match case {
        "canonical_prefix" => {
            br#"<?xpacket begin=""?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:pdfuaid="http://www.aiim.org/pdfua/ns/id/" xmlns:dc="http://purl.org/dc/elements/1.1/"><rdf:Description pdfuaid:part="1" pdfuaid:corr="1:2014"><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Document title</rdf:li></rdf:Alt></dc:title></rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end="w"?>"#
        }
        "wrong_prefix" => {
            br#"<?xpacket begin=""?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:pdfuaid="http://www.aiim.org/pdfua/ns/id/" xmlns:wrong="http://www.aiim.org/pdfua/ns/id/" xmlns:dc="http://purl.org/dc/elements/1.1/"><rdf:Description pdfuaid:part="1" wrong:corr="1:2014"><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Document title</rdf:li></rdf:Alt></dc:title></rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end="w"?>"#
        }
        _ => panic!("unknown PDF/UA-1 rule 5-5 fixture case {case}"),
    };

    let mut document = pdf_document();
    let pages_id = document.new_object_id();
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });
    wrap_pages(&mut document, pages_id, page_id);
    let metadata_id = document.add_object(Stream::new(
        dictionary! {
            "Type" => "Metadata",
            "Subtype" => "XML",
        },
        xmp.to_vec(),
    ));
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "MarkInfo" => dictionary! { "Marked" => true },
        "ViewerPreferences" => dictionary! { "DisplayDocTitle" => true },
        "StructTreeRoot" => Dictionary::new(),
    });
    document.trailer.set("Root", catalog_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 5-5 fixture");
    bytes
}

pub fn pdfua1_rule_6_1_fixture(case: &str) -> Vec<u8> {
    let mut bytes = pdfua1_rule_5_1_fixture("identification_present");
    if case == "invalid_header" {
        replace_once(&mut bytes, b"%PDF-1.4", b"%PDF-1.8");
    } else if case != "valid_header" {
        panic!("unknown PDF/UA-1 rule 6.1-1 fixture case {case}");
    }
    bytes
}

pub fn pdfua1_rule_6_2_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_5_1_fixture("identification_present"))
        .expect("load PDF/UA-1 rule 6.2-1 fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let catalog = document
        .get_object_mut(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture catalog dictionary");
    match case {
        "marked_true" => catalog.set("MarkInfo", dictionary! { "Marked" => true }),
        "marked_false" => catalog.set("MarkInfo", dictionary! { "Marked" => false }),
        _ => panic!("unknown PDF/UA-1 rule 6.2-1 fixture case {case}"),
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 6.2-1 fixture");
    bytes
}

pub fn pdfua1_rule_7_1_4_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_5_1_fixture("identification_present"))
        .expect("load PDF/UA-1 rule 7.1-4 fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let catalog = document
        .get_object_mut(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture catalog dictionary");
    match case {
        "false" => catalog.set(
            "MarkInfo",
            dictionary! { "Marked" => true, "Suspects" => false },
        ),
        "true" => catalog.set(
            "MarkInfo",
            dictionary! { "Marked" => true, "Suspects" => true },
        ),
        _ => panic!("unknown PDF/UA-1 rule 7.1-4 fixture case {case}"),
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.1-4 fixture");
    bytes
}

pub fn pdfua1_rule_7_1_8_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_5_1_fixture("identification_present"))
        .expect("load PDF/UA-1 rule 7.1-8 fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let catalog = document
        .get_object_mut(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture catalog dictionary");
    let metadata_id = catalog
        .get(b"Metadata")
        .expect("PDF/UA-1 fixture metadata")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture metadata");
    match case {
        "valid" => {}
        "missing" => {
            catalog.remove(b"Metadata");
        }
        "wrong_type" => document
            .get_object_mut(metadata_id)
            .expect("PDF/UA-1 fixture metadata stream")
            .as_stream_mut()
            .expect("PDF/UA-1 fixture metadata stream")
            .dict
            .set("Type", "NotMetadata"),
        "wrong_subtype" => document
            .get_object_mut(metadata_id)
            .expect("PDF/UA-1 fixture metadata stream")
            .as_stream_mut()
            .expect("PDF/UA-1 fixture metadata stream")
            .dict
            .set("Subtype", "NotXML"),
        _ => panic!("unknown PDF/UA-1 rule 7.1-8 fixture case {case}"),
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.1-8 fixture");
    bytes
}

pub fn pdfua1_rule_7_1_9_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_5_1_fixture("identification_present"))
        .expect("load PDF/UA-1 rule 7.1-9 fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let catalog = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary");
    let metadata_id = catalog
        .get(b"Metadata")
        .expect("PDF/UA-1 fixture metadata")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture metadata");
    if case == "missing" {
        let metadata = document
            .get_object_mut(metadata_id)
            .expect("PDF/UA-1 fixture metadata stream")
            .as_stream_mut()
            .expect("PDF/UA-1 fixture metadata stream");
        replace(
            &mut metadata.content,
            "<dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">Document title</rdf:li></rdf:Alt></dc:title>",
            "",
        );
        metadata.set_content(metadata.content.clone());
    } else if case != "present" {
        panic!("unknown PDF/UA-1 rule 7.1-9 fixture case {case}");
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.1-9 fixture");
    bytes
}

pub fn pdfua1_rule_7_1_10_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_5_1_fixture("identification_present"))
        .expect("load PDF/UA-1 rule 7.1-10 fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let catalog = document
        .get_object_mut(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture catalog dictionary");
    match case {
        "present" => catalog.set(
            "ViewerPreferences",
            dictionary! { "DisplayDocTitle" => true },
        ),
        "false" => catalog.set(
            "ViewerPreferences",
            dictionary! { "DisplayDocTitle" => false },
        ),
        "missing" => {
            catalog.remove(b"ViewerPreferences");
        }
        _ => panic!("unknown PDF/UA-1 rule 7.1-10 fixture case {case}"),
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.1-10 fixture");
    bytes
}

pub fn pdfua1_rule_7_1_11_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_5_1_fixture("identification_present"))
        .expect("load PDF/UA-1 rule 7.1-11 fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let catalog = document
        .get_object_mut(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture catalog dictionary");
    match case {
        "present" => {}
        "missing" => {
            catalog.remove(b"StructTreeRoot");
        }
        _ => panic!("unknown PDF/UA-1 rule 7.1-11 fixture case {case}"),
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.1-11 fixture");
    bytes
}

pub fn pdfua1_rule_7_1_12_fixture(case: &str) -> Vec<u8> {
    let base = Document::load_mem(&pdfua1_rule_5_1_fixture("identification_present"))
        .expect("load PDF/UA-1 metadata fixture");
    let base_root_id = base
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 base fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 base fixture root");
    let base_catalog = base
        .get_object(base_root_id)
        .expect("PDF/UA-1 base fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 base fixture catalog dictionary");
    let metadata_id = base_catalog
        .get(b"Metadata")
        .expect("PDF/UA-1 base fixture metadata")
        .as_reference()
        .expect("indirect PDF/UA-1 base fixture metadata");
    let xmp = base
        .get_object(metadata_id)
        .expect("PDF/UA-1 base fixture metadata object")
        .as_stream()
        .expect("PDF/UA-1 base fixture metadata stream")
        .content
        .clone();

    let mut document = Document::load_mem(include_bytes!(
        "../fixtures/canonical-pdfa-1a-structure.pdf"
    ))
    .expect("load tagged PDF/UA-1 rule 7.1-12 fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let (metadata_id, struct_tree_root_id) = {
        let catalog = document
            .get_object_mut(root_id)
            .expect("PDF/UA-1 fixture catalog")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture catalog dictionary");
        catalog.set("MarkInfo", dictionary! { "Marked" => true });
        catalog.set(
            "ViewerPreferences",
            dictionary! { "DisplayDocTitle" => true },
        );
        let metadata_id = catalog
            .get(b"Metadata")
            .expect("PDF/UA-1 fixture metadata")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture metadata");
        let struct_tree_root_id = catalog
            .get(b"StructTreeRoot")
            .expect("PDF/UA-1 fixture structure tree root")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture structure tree root");
        (metadata_id, struct_tree_root_id)
    };
    document
        .get_object_mut(metadata_id)
        .expect("PDF/UA-1 fixture metadata object")
        .as_stream_mut()
        .expect("PDF/UA-1 fixture metadata stream")
        .set_content(xmp);
    if case == "missing" {
        let top_level_structure_element_id = document
            .get_object(struct_tree_root_id)
            .expect("PDF/UA-1 fixture structure tree root object")
            .as_dict()
            .expect("PDF/UA-1 fixture structure tree root dictionary")
            .get(b"K")
            .expect("PDF/UA-1 fixture structure tree root kids")
            .as_array()
            .expect("PDF/UA-1 fixture structure tree root kids array")
            .first()
            .expect("PDF/UA-1 fixture structure tree root first kid")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture structure element");
        let structure_element_id = document
            .get_object(top_level_structure_element_id)
            .expect("PDF/UA-1 fixture top-level structure element")
            .as_dict()
            .expect("PDF/UA-1 fixture top-level structure element dictionary")
            .get(b"K")
            .expect("PDF/UA-1 fixture top-level structure element kids")
            .as_array()
            .expect("PDF/UA-1 fixture top-level structure element kids array")
            .first()
            .expect("PDF/UA-1 fixture top-level structure element first kid")
            .as_reference()
            .expect("indirect PDF/UA-1 nested structure element");
        document
            .get_object_mut(structure_element_id)
            .expect("PDF/UA-1 fixture structure element")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture structure element dictionary")
            .remove(b"P");
    } else if case != "present" {
        panic!("unknown PDF/UA-1 rule 7.1-12 fixture case {case}");
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.1-12 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_2_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 rule 7.2-2 fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let outlines_id = document.new_object_id();
    let outline_id = document.add_object(dictionary! {
        "Title" => Object::string_literal("Outline entry"),
        "Parent" => outlines_id,
    });
    document.objects.insert(
        outlines_id,
        Object::Dictionary(dictionary! {
            "Type" => "Outlines",
            "First" => outline_id,
            "Last" => outline_id,
            "Count" => 1,
        }),
    );
    let catalog = document
        .get_object_mut(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture catalog dictionary");
    catalog.set("Outlines", outlines_id);
    if case == "language_missing" {
        catalog.remove(b"Lang");
    } else if case != "language_present" {
        panic!("unknown PDF/UA-1 rule 7.2-2 fixture case {case}");
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-2 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_21_fixture(case: &str) -> Vec<u8> {
    pdfua1_rule_7_2_text_language_fixture(case, "ActualText")
}

pub fn pdfua1_rule_7_2_22_fixture(case: &str) -> Vec<u8> {
    pdfua1_rule_7_2_text_language_fixture(case, "Alt")
}

pub fn pdfua1_rule_7_2_23_fixture(case: &str) -> Vec<u8> {
    pdfua1_rule_7_2_text_language_fixture(case, "E")
}

pub fn pdfua1_rule_7_2_30_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 Span ActualText language fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let pages_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"Pages")
        .expect("PDF/UA-1 fixture pages")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture pages");
    let page_id = document
        .get_object(pages_id)
        .expect("PDF/UA-1 fixture pages object")
        .as_dict()
        .expect("PDF/UA-1 fixture pages dictionary")
        .get(b"Kids")
        .expect("PDF/UA-1 fixture page kids")
        .as_array()
        .expect("PDF/UA-1 fixture page kids array")
        .first()
        .expect("PDF/UA-1 fixture page")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture page");
    let (catalog_language, content) = match case {
        "property_language_present" => (
            false,
            b"/Span <</MCID 0 /ActualText (replacement) /Lang (en)>> BDC\nBT /F1 12 Tf (x) Tj ET\nEMC\n"
                .as_slice(),
        ),
        "inherited_language_present" => (
            false,
            b"/P <</MCID 0 /Lang (en)>> BDC\n/Span <</MCID 1 /ActualText (replacement)>> BDC\nBT /F1 12 Tf (x) Tj ET\nEMC\nEMC\n"
                .as_slice(),
        ),
        "catalog_language_present" => (
            true,
            b"/Span <</MCID 0 /ActualText (replacement)>> BDC\nBT /F1 12 Tf (x) Tj ET\nEMC\n"
                .as_slice(),
        ),
        "language_missing" => (
            false,
            b"/Span <</MCID 0 /ActualText (replacement)>> BDC\nBT /F1 12 Tf (x) Tj ET\nEMC\n"
                .as_slice(),
        ),
        _ => panic!("unknown PDF/UA-1 rule 7.2-30 fixture case {case}"),
    };
    let catalog = document
        .get_object_mut(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture catalog dictionary");
    catalog.remove(b"Outlines");
    if catalog_language {
        catalog.set("Lang", Object::string_literal("en"));
    } else {
        catalog.remove(b"Lang");
    }
    let contents_id = document.add_object(Stream::new(Dictionary::new(), content.to_vec()));
    document
        .get_object_mut(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture page dictionary")
        .set("Contents", contents_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 Span ActualText language fixture");
    bytes
}

pub fn pdfua1_rule_7_2_31_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 Span Alt language fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let pages_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"Pages")
        .expect("PDF/UA-1 fixture pages")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture pages");
    let page_id = document
        .get_object(pages_id)
        .expect("PDF/UA-1 fixture pages object")
        .as_dict()
        .expect("PDF/UA-1 fixture pages dictionary")
        .get(b"Kids")
        .expect("PDF/UA-1 fixture page kids")
        .as_array()
        .expect("PDF/UA-1 fixture page kids array")
        .first()
        .expect("PDF/UA-1 fixture page")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture page");
    let (catalog_language, content) = match case {
        "property_language_present" => (
            false,
            b"/Span <</MCID 0 /Alt (alternative) /Lang (en)>> BDC\nBT /F1 12 Tf (x) Tj ET\nEMC\n"
                .as_slice(),
        ),
        "inherited_language_present" => (
            false,
            b"/P <</MCID 0 /Lang (en)>> BDC\n/Span <</MCID 1 /Alt (alternative)>> BDC\nBT /F1 12 Tf (x) Tj ET\nEMC\nEMC\n"
                .as_slice(),
        ),
        "catalog_language_present" => (
            true,
            b"/Span <</MCID 0 /Alt (alternative)>> BDC\nBT /F1 12 Tf (x) Tj ET\nEMC\n"
                .as_slice(),
        ),
        "language_missing" => (
            false,
            b"/Span <</MCID 0 /Alt (alternative)>> BDC\nBT /F1 12 Tf (x) Tj ET\nEMC\n"
                .as_slice(),
        ),
        _ => panic!("unknown PDF/UA-1 rule 7.2-31 fixture case {case}"),
    };
    let catalog = document
        .get_object_mut(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture catalog dictionary");
    catalog.remove(b"Outlines");
    if catalog_language {
        catalog.set("Lang", Object::string_literal("en"));
    } else {
        catalog.remove(b"Lang");
    }
    let contents_id = document.add_object(Stream::new(Dictionary::new(), content.to_vec()));
    document
        .get_object_mut(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture page dictionary")
        .set("Contents", contents_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 Span Alt language fixture");
    bytes
}

pub fn pdfua1_rule_7_2_32_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 Span E language fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let pages_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"Pages")
        .expect("PDF/UA-1 fixture pages")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture pages");
    let page_id = document
        .get_object(pages_id)
        .expect("PDF/UA-1 fixture pages object")
        .as_dict()
        .expect("PDF/UA-1 fixture pages dictionary")
        .get(b"Kids")
        .expect("PDF/UA-1 fixture page kids")
        .as_array()
        .expect("PDF/UA-1 fixture page kids array")
        .first()
        .expect("PDF/UA-1 fixture page")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture page");
    let (catalog_language, content) = match case {
        "property_language_present" => (
            false,
            b"/Span <</MCID 0 /E (expansion) /Lang (en)>> BDC\nBT /F1 12 Tf (x) Tj ET\nEMC\n"
                .as_slice(),
        ),
        "inherited_language_present" => (
            false,
            b"/P <</MCID 0 /Lang (en)>> BDC\n/Span <</MCID 1 /E (expansion)>> BDC\nBT /F1 12 Tf (x) Tj ET\nEMC\nEMC\n"
                .as_slice(),
        ),
        "catalog_language_present" => (
            true,
            b"/Span <</MCID 0 /E (expansion)>> BDC\nBT /F1 12 Tf (x) Tj ET\nEMC\n"
                .as_slice(),
        ),
        "language_missing" => (
            false,
            b"/Span <</MCID 0 /E (expansion)>> BDC\nBT /F1 12 Tf (x) Tj ET\nEMC\n"
                .as_slice(),
        ),
        _ => panic!("unknown PDF/UA-1 rule 7.2-32 fixture case {case}"),
    };
    let catalog = document
        .get_object_mut(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture catalog dictionary");
    catalog.remove(b"Outlines");
    if catalog_language {
        catalog.set("Lang", Object::string_literal("en"));
    } else {
        catalog.remove(b"Lang");
    }
    let contents_id = document.add_object(Stream::new(Dictionary::new(), content.to_vec()));
    document
        .get_object_mut(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture page dictionary")
        .set("Contents", contents_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 Span E language fixture");
    bytes
}

pub fn pdfua1_rule_7_2_33_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 metadata language fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let metadata_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"Metadata")
        .expect("PDF/UA-1 fixture metadata")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture metadata");
    let xmp: &[u8] = match case {
        "x_default" | "catalog_language" => {
            br#"<?xpacket begin=""?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:pdfuaid="http://www.aiim.org/pdfua/ns/id/" xmlns:dc="http://purl.org/dc/elements/1.1/"><rdf:Description pdfuaid:part="1"><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Document title</rdf:li></rdf:Alt></dc:title></rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end="w"?>"#
        }
        "multiple_items" => {
            br#"<?xpacket begin=""?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:pdfuaid="http://www.aiim.org/pdfua/ns/id/" xmlns:dc="http://purl.org/dc/elements/1.1/"><rdf:Description pdfuaid:part="1"><dc:title><rdf:Alt><rdf:li xml:lang="fr">Titre</rdf:li><rdf:li xml:lang="x-default">Document title</rdf:li></rdf:Alt></dc:title></rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end="w"?>"#
        }
        "missing_x_default" => {
            br#"<?xpacket begin=""?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:pdfuaid="http://www.aiim.org/pdfua/ns/id/" xmlns:dc="http://purl.org/dc/elements/1.1/"><rdf:Description pdfuaid:part="1"><dc:title><rdf:Alt><rdf:li xml:lang="en">Document title</rdf:li></rdf:Alt></dc:title></rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end="w"?>"#
        }
        _ => panic!("unknown PDF/UA-1 rule 7.2-33 fixture case {case}"),
    };
    let catalog = document
        .get_object_mut(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture catalog dictionary");
    catalog.remove(b"Outlines");
    if case == "catalog_language" {
        catalog.set("Lang", Object::string_literal("en"));
    } else {
        catalog.remove(b"Lang");
    }
    document
        .get_object_mut(metadata_id)
        .expect("PDF/UA-1 fixture metadata object")
        .as_stream_mut()
        .expect("PDF/UA-1 fixture metadata stream")
        .set_content(xmp.to_vec());
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-33 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_34_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 page text language fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let pages_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"Pages")
        .expect("PDF/UA-1 fixture pages")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture pages");
    let page_id = document
        .get_object(pages_id)
        .expect("PDF/UA-1 fixture pages object")
        .as_dict()
        .expect("PDF/UA-1 fixture pages dictionary")
        .get(b"Kids")
        .expect("PDF/UA-1 fixture page kids")
        .as_array()
        .expect("PDF/UA-1 fixture page kids array")
        .first()
        .expect("PDF/UA-1 fixture page")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture page");
    let (catalog_language, content) = match case {
        "property_language_present" => (
            false,
            b"/Span <</MCID 0 /Lang (en)>> BDC\nBT /F1 12 Tf (x) Tj ET\nEMC\n".as_slice(),
        ),
        "inherited_language_present" => (
            false,
            b"/P <</MCID 0 /Lang (en)>> BDC\n/Span <</MCID 1>> BDC\nBT /F1 12 Tf (x) Tj ET\nEMC\nEMC\n"
                .as_slice(),
        ),
        "catalog_language_present" => (
            true,
            b"/Span <</MCID 0>> BDC\nBT /F1 12 Tf (x) Tj ET\nEMC\n".as_slice(),
        ),
        "language_missing" => (
            false,
            b"/Span <</MCID 0>> BDC\nBT /F1 12 Tf (x) Tj ET\nEMC\n".as_slice(),
        ),
        _ => panic!("unknown PDF/UA-1 rule 7.2-34 fixture case {case}"),
    };
    let catalog = document
        .get_object_mut(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture catalog dictionary");
    catalog.remove(b"Outlines");
    if catalog_language {
        catalog.set("Lang", Object::string_literal("en"));
    } else {
        catalog.remove(b"Lang");
    }
    let contents_id = document.add_object(Stream::new(Dictionary::new(), content.to_vec()));
    document
        .get_object_mut(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture page dictionary")
        .set("Contents", contents_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 page text language fixture");
    bytes
}

pub fn pdfua1_rule_7_2_25_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 form-field language fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let mut field = dictionary! {
        "T" => Object::string_literal("field"),
        "FT" => "Tx",
    };
    let catalog_language_present = match case {
        "tu_absent" => false,
        "tu_present_catalog_language" => {
            field.set("TU", Object::string_literal("Field help"));
            true
        }
        "tu_present_language_missing" => {
            field.set("TU", Object::string_literal("Field help"));
            false
        }
        _ => panic!("unknown PDF/UA-1 rule 7.2-25 fixture case {case}"),
    };
    let field_id = document.add_object(field);
    let acro_form_id = document.add_object(dictionary! {
        "Fields" => vec![Object::Reference(field_id)],
    });
    let catalog = document
        .get_object_mut(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture catalog dictionary");
    catalog.remove(b"Outlines");
    catalog.remove(b"Lang");
    catalog.set("AcroForm", acro_form_id);
    if catalog_language_present {
        catalog.set("Lang", Object::string_literal("en"));
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 form-field language fixture");
    bytes
}

pub fn pdfua1_rule_7_2_26_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 TOCI containment fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let toci_id = document.new_object_id();
    let parent_id = match case {
        "contained" => {
            let toc_id = document.new_object_id();
            document.objects.insert(
                toc_id,
                Object::Dictionary(dictionary! {
                    "S" => "TOC",
                    "P" => struct_tree_root_id,
                    "K" => vec![Object::Reference(toci_id)],
                }),
            );
            toc_id
        }
        "not_contained" => struct_tree_root_id,
        _ => panic!("unknown PDF/UA-1 rule 7.2-26 fixture case {case}"),
    };
    let top_level_id = if case == "contained" {
        parent_id
    } else {
        toci_id
    };
    document.objects.insert(
        toci_id,
        Object::Dictionary(dictionary! {
            "S" => "TOCI",
            "P" => parent_id,
        }),
    );
    let struct_tree_root = document
        .get_object_mut(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture structure tree root dictionary");
    struct_tree_root
        .get_mut(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array_mut()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .push(Object::Reference(top_level_id));
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-26 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_27_fixture(case: &str) -> Vec<u8> {
    let child_types = match case {
        "allowed" => ["Caption", "TOC", "TOCI"].as_slice(),
        "invalid" => ["P"].as_slice(),
        _ => panic!("unknown PDF/UA-1 rule 7.2-27 fixture case {case}"),
    };
    pdfua1_toc_fixture(child_types)
}

pub fn pdfua1_rule_7_2_28_fixture(case: &str) -> Vec<u8> {
    let child_types = match case {
        "caption_first" => ["Caption", "TOCI"].as_slice(),
        "caption_not_first" => ["TOCI", "Caption"].as_slice(),
        _ => panic!("unknown PDF/UA-1 rule 7.2-28 fixture case {case}"),
    };
    pdfua1_toc_fixture(child_types)
}

pub fn pdfua1_rule_7_2_40_fixture(case: &str) -> Vec<u8> {
    let child_types = match case {
        "caption_first" => ["Caption", "LI"].as_slice(),
        "caption_not_first" => ["LI", "Caption"].as_slice(),
        _ => panic!("unknown PDF/UA-1 rule 7.2-40 fixture case {case}"),
    };
    pdfua1_list_fixture(child_types)
}

pub fn pdfua1_rule_7_2_29_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 language-tag fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let (struct_tree_root_id, page_id) = {
        let catalog = document
            .get_object(root_id)
            .expect("PDF/UA-1 fixture catalog")
            .as_dict()
            .expect("PDF/UA-1 fixture catalog dictionary");
        let pages_id = catalog
            .get(b"Pages")
            .expect("PDF/UA-1 fixture pages")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture pages");
        let page_id = document
            .get_object(pages_id)
            .expect("PDF/UA-1 fixture pages object")
            .as_dict()
            .expect("PDF/UA-1 fixture pages dictionary")
            .get(b"Kids")
            .expect("PDF/UA-1 fixture page kids")
            .as_array()
            .expect("PDF/UA-1 fixture page kids array")
            .first()
            .expect("PDF/UA-1 fixture page")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture page");
        let struct_tree_root_id = catalog
            .get(b"StructTreeRoot")
            .expect("PDF/UA-1 fixture structure tree root")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture structure tree root");
        (struct_tree_root_id, page_id)
    };
    let structure_element_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .first()
        .expect("PDF/UA-1 fixture structure tree root first kid")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure element");

    let invalid_value = Object::string_literal("en_US");
    let valid_value = Object::string_literal("en-US");
    match case {
        "catalog_valid" | "catalog_invalid" => document
            .get_object_mut(root_id)
            .expect("PDF/UA-1 fixture catalog")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture catalog dictionary")
            .set(
                "Lang",
                if case == "catalog_valid" {
                    valid_value
                } else {
                    invalid_value
                },
            ),
        "structure_valid" | "structure_invalid" => document
            .get_object_mut(structure_element_id)
            .expect("PDF/UA-1 fixture structure element")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture structure element dictionary")
            .set(
                "Lang",
                if case == "structure_valid" {
                    valid_value
                } else {
                    invalid_value
                },
            ),
        "property_valid" | "property_invalid" => {
            let contents_id = document
                .get_object(page_id)
                .expect("PDF/UA-1 fixture page")
                .as_dict()
                .expect("PDF/UA-1 fixture page dictionary")
                .get(b"Contents")
                .expect("PDF/UA-1 fixture page contents")
                .as_reference()
                .expect("indirect PDF/UA-1 fixture page contents");
            let contents = document
                .get_object_mut(contents_id)
                .expect("PDF/UA-1 fixture page contents stream")
                .as_stream_mut()
                .expect("PDF/UA-1 fixture page contents stream");
            let mut content = contents
                .decompressed_content()
                .expect("decompress PDF/UA-1 fixture page contents");
            let language = if case == "property_valid" {
                b"en-US".as_slice()
            } else {
                b"en_US".as_slice()
            };
            replace_once(
                &mut content,
                b"/Span<</MCID 0>>BDC",
                [b"/Span<</MCID 0 /Lang (".as_slice(), language, b")>>BDC"]
                    .concat()
                    .as_slice(),
            );
            contents.dict.remove(b"Filter");
            contents.dict.remove(b"DecodeParms");
            contents.set_content(content);
        }
        _ => panic!("unknown PDF/UA-1 rule 7.2-29 fixture case {case}"),
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-29 fixture");
    bytes
}

fn pdfua1_toc_fixture(child_types: &[&str]) -> Vec<u8> {
    pdfua1_structure_fixture("TOC", child_types)
}

fn pdfua1_structure_fixture(structure_type: &str, child_types: &[&str]) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 TOC children fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");

    let child_ids = child_types
        .iter()
        .map(|child_type| {
            document.add_object(dictionary! {
                "S" => *child_type,
                "P" => Object::Reference(struct_tree_root_id),
            })
        })
        .collect::<Vec<_>>();
    let toc_id = document.add_object(dictionary! {
        "S" => structure_type,
        "P" => Object::Reference(struct_tree_root_id),
        "K" => child_ids
            .iter()
            .copied()
            .map(Object::Reference)
            .collect::<Vec<_>>(),
    });
    for child_id in child_ids {
        document
            .get_object_mut(child_id)
            .expect("PDF/UA-1 fixture TOC child")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture TOC child dictionary")
            .set("P", Object::Reference(toc_id));
    }
    document
        .get_object_mut(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get_mut(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array_mut()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .push(Object::Reference(toc_id));

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 structure fixture");
    bytes
}

fn pdfua1_list_fixture(child_types: &[&str]) -> Vec<u8> {
    pdfua1_structure_fixture("L", child_types)
}

pub fn pdfua1_rule_7_2_17_fixture(case: &str) -> Vec<u8> {
    if case == "contained" {
        return pdfua1_list_fixture(&["LI"]);
    }

    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 LI parent fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let li_id = document.add_object(dictionary! {
        "S" => "LI",
        "P" => Object::Reference(struct_tree_root_id),
    });
    document
        .get_object_mut(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get_mut(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array_mut()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .push(Object::Reference(li_id));

    match case {
        "not_contained" => {}
        _ => panic!("unknown PDF/UA-1 rule 7.2-17 fixture case {case}"),
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-17 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_18_fixture(case: &str) -> Vec<u8> {
    if case == "contained" {
        let mut document = Document::load_mem(&pdfua1_rule_7_2_17_fixture("contained"))
            .expect("load PDF/UA-1 LBody parent fixture");
        let root_id = document
            .trailer
            .get(b"Root")
            .expect("PDF/UA-1 fixture root")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture root");
        let struct_tree_root_id = document
            .get_object(root_id)
            .expect("PDF/UA-1 fixture catalog")
            .as_dict()
            .expect("PDF/UA-1 fixture catalog dictionary")
            .get(b"StructTreeRoot")
            .expect("PDF/UA-1 fixture structure tree root")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture structure tree root");
        let list_id = document
            .get_object(struct_tree_root_id)
            .expect("PDF/UA-1 fixture structure tree root")
            .as_dict()
            .expect("PDF/UA-1 fixture structure tree root dictionary")
            .get(b"K")
            .expect("PDF/UA-1 fixture structure tree root kids")
            .as_array()
            .expect("PDF/UA-1 fixture structure tree root kids array")
            .last()
            .expect("PDF/UA-1 fixture list")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture list");
        let li_id = document
            .get_object(list_id)
            .expect("PDF/UA-1 fixture list")
            .as_dict()
            .expect("PDF/UA-1 fixture list dictionary")
            .get(b"K")
            .expect("PDF/UA-1 fixture list kids")
            .as_array()
            .expect("PDF/UA-1 fixture list kids array")
            .first()
            .expect("PDF/UA-1 fixture list item")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture list item");
        let lbody_id = document.add_object(dictionary! {
            "S" => "LBody",
            "P" => Object::Reference(li_id),
        });
        document
            .get_object_mut(li_id)
            .expect("PDF/UA-1 fixture list item")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture list item dictionary")
            .set("K", vec![Object::Reference(lbody_id)]);
        let mut bytes = Vec::new();
        document
            .save_to(&mut bytes)
            .expect("save PDF/UA-1 rule 7.2-18 fixture");
        return bytes;
    }

    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 LBody parent fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let lbody_id = document.add_object(dictionary! {
        "S" => "LBody",
        "P" => Object::Reference(struct_tree_root_id),
    });
    document
        .get_object_mut(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get_mut(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array_mut()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .push(Object::Reference(lbody_id));

    match case {
        "not_contained" => {}
        _ => panic!("unknown PDF/UA-1 rule 7.2-18 fixture case {case}"),
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-18 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_19_fixture(case: &str) -> Vec<u8> {
    let child_types = match case {
        "allowed" => ["Caption", "L", "LI"].as_slice(),
        "invalid" => ["P"].as_slice(),
        _ => panic!("unknown PDF/UA-1 rule 7.2-19 fixture case {case}"),
    };
    pdfua1_structure_fixture("L", child_types)
}

pub fn pdfua1_rule_7_2_20_fixture(case: &str) -> Vec<u8> {
    let child_types = match case {
        "allowed" => ["Lbl", "LBody"].as_slice(),
        "invalid" => ["P"].as_slice(),
        _ => panic!("unknown PDF/UA-1 rule 7.2-20 fixture case {case}"),
    };
    let mut document = Document::load_mem(&pdfua1_rule_7_2_17_fixture("contained"))
        .expect("load PDF/UA-1 list item children fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let list_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .last()
        .expect("PDF/UA-1 fixture list")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture list");
    let li_id = document
        .get_object(list_id)
        .expect("PDF/UA-1 fixture list")
        .as_dict()
        .expect("PDF/UA-1 fixture list dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture list kids")
        .as_array()
        .expect("PDF/UA-1 fixture list kids array")
        .first()
        .expect("PDF/UA-1 fixture list item")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture list item");
    let child_ids = child_types
        .iter()
        .map(|child_type| {
            document.add_object(dictionary! {
                "S" => *child_type,
                "P" => Object::Reference(li_id),
            })
        })
        .collect::<Vec<_>>();
    document
        .get_object_mut(li_id)
        .expect("PDF/UA-1 fixture list item")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture list item dictionary")
        .set(
            "K",
            child_ids
                .iter()
                .copied()
                .map(Object::Reference)
                .collect::<Vec<_>>(),
        );
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-20 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_3_fixture(case: &str) -> Vec<u8> {
    let child_types = match case {
        "allowed" => ["TR", "THead", "TBody", "TFoot", "Caption"].as_slice(),
        "invalid" => ["P"].as_slice(),
        _ => panic!("unknown PDF/UA-1 rule 7.2-3 fixture case {case}"),
    };
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 table children fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let child_ids = child_types
        .iter()
        .map(|child_type| {
            document.add_object(dictionary! {
                "S" => *child_type,
                "P" => Object::Reference(struct_tree_root_id),
            })
        })
        .collect::<Vec<_>>();
    let table_id = document.add_object(dictionary! {
        "S" => "Table",
        "P" => Object::Reference(struct_tree_root_id),
        "K" => child_ids
            .iter()
            .copied()
            .map(Object::Reference)
            .collect::<Vec<_>>(),
    });
    for child_id in child_ids {
        document
            .get_object_mut(child_id)
            .expect("PDF/UA-1 fixture table child")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture table child dictionary")
            .set("P", Object::Reference(table_id));
    }
    document
        .get_object_mut(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get_mut(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array_mut()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .push(Object::Reference(table_id));
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-3 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_4_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_2_3_fixture("allowed"))
        .expect("load PDF/UA-1 TR parent fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");

    let table_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .last()
        .expect("PDF/UA-1 fixture table")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture table");

    match case {
        "contained" => {
            let section_ids = document
                .get_object(table_id)
                .expect("PDF/UA-1 fixture table")
                .as_dict()
                .expect("PDF/UA-1 fixture table dictionary")
                .get(b"K")
                .expect("PDF/UA-1 fixture table kids")
                .as_array()
                .expect("PDF/UA-1 fixture table kids array")
                .iter()
                .filter_map(|kid| {
                    let kid_id = kid.as_reference().ok()?;
                    let structure_type = document
                        .get_object(kid_id)
                        .ok()?
                        .as_dict()
                        .ok()?
                        .get(b"S")
                        .ok()?
                        .as_name()
                        .ok()?;
                    matches!(structure_type, b"THead" | b"TBody" | b"TFoot").then_some(kid_id)
                })
                .collect::<Vec<_>>();
            for section_id in section_ids {
                let row_id = document.add_object(dictionary! {
                    "S" => "TR",
                    "P" => Object::Reference(section_id),
                });
                document
                    .get_object_mut(section_id)
                    .expect("PDF/UA-1 fixture table section")
                    .as_dict_mut()
                    .expect("PDF/UA-1 fixture table section dictionary")
                    .set("K", vec![Object::Reference(row_id)]);
            }
        }
        "not_contained" => {
            let row_id = document.add_object(dictionary! {
                "S" => "TR",
                "P" => Object::Reference(struct_tree_root_id),
            });
            document
                .get_object_mut(struct_tree_root_id)
                .expect("PDF/UA-1 fixture structure tree root")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture structure tree root dictionary")
                .get_mut(b"K")
                .expect("PDF/UA-1 fixture structure tree root kids")
                .as_array_mut()
                .expect("PDF/UA-1 fixture structure tree root kids array")
                .push(Object::Reference(row_id));
        }
        _ => panic!("unknown PDF/UA-1 rule 7.2-4 fixture case {case}"),
    }

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-4 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_5_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_2_3_fixture("allowed"))
        .expect("load PDF/UA-1 THead parent fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");

    match case {
        "contained" => {}
        "not_contained" => {
            let thead_id = document.add_object(dictionary! {
                "S" => "THead",
                "P" => Object::Reference(struct_tree_root_id),
            });
            document
                .get_object_mut(struct_tree_root_id)
                .expect("PDF/UA-1 fixture structure tree root")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture structure tree root dictionary")
                .get_mut(b"K")
                .expect("PDF/UA-1 fixture structure tree root kids")
                .as_array_mut()
                .expect("PDF/UA-1 fixture structure tree root kids array")
                .push(Object::Reference(thead_id));
        }
        _ => panic!("unknown PDF/UA-1 rule 7.2-5 fixture case {case}"),
    }

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-5 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_6_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_2_3_fixture("allowed"))
        .expect("load PDF/UA-1 TBody parent fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");

    match case {
        "contained" => {}
        "not_contained" => {
            let tbody_id = document.add_object(dictionary! {
                "S" => "TBody",
                "P" => Object::Reference(struct_tree_root_id),
            });
            document
                .get_object_mut(struct_tree_root_id)
                .expect("PDF/UA-1 fixture structure tree root")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture structure tree root dictionary")
                .get_mut(b"K")
                .expect("PDF/UA-1 fixture structure tree root kids")
                .as_array_mut()
                .expect("PDF/UA-1 fixture structure tree root kids array")
                .push(Object::Reference(tbody_id));
        }
        _ => panic!("unknown PDF/UA-1 rule 7.2-6 fixture case {case}"),
    }

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-6 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_7_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_2_3_fixture("allowed"))
        .expect("load PDF/UA-1 TFoot parent fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");

    match case {
        "contained" => {}
        "not_contained" => {
            let tfoot_id = document.add_object(dictionary! {
                "S" => "TFoot",
                "P" => Object::Reference(struct_tree_root_id),
            });
            document
                .get_object_mut(struct_tree_root_id)
                .expect("PDF/UA-1 fixture structure tree root")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture structure tree root dictionary")
                .get_mut(b"K")
                .expect("PDF/UA-1 fixture structure tree root kids")
                .as_array_mut()
                .expect("PDF/UA-1 fixture structure tree root kids array")
                .push(Object::Reference(tfoot_id));
        }
        _ => panic!("unknown PDF/UA-1 rule 7.2-7 fixture case {case}"),
    }

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-7 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_8_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_2_3_fixture("allowed"))
        .expect("load PDF/UA-1 TH parent fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");

    match case {
        "contained" => {}
        "not_contained" => {
            let th_id = document.add_object(dictionary! {
                "S" => "TH",
                "P" => Object::Reference(struct_tree_root_id),
            });
            document
                .get_object_mut(struct_tree_root_id)
                .expect("PDF/UA-1 fixture structure tree root")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture structure tree root dictionary")
                .get_mut(b"K")
                .expect("PDF/UA-1 fixture structure tree root kids")
                .as_array_mut()
                .expect("PDF/UA-1 fixture structure tree root kids array")
                .push(Object::Reference(th_id));
        }
        _ => panic!("unknown PDF/UA-1 rule 7.2-8 fixture case {case}"),
    }

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-8 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_9_fixture(case: &str) -> Vec<u8> {
    let source = match case {
        "contained" => pdfua1_rule_7_2_4_fixture("contained"),
        "not_contained" => pdfua1_rule_7_2_3_fixture("allowed"),
        _ => panic!("unknown PDF/UA-1 rule 7.2-9 fixture case {case}"),
    };
    let mut document = Document::load_mem(&source).expect("load PDF/UA-1 TD parent fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");

    match case {
        "contained" => {
            let table_id = document
                .get_object(struct_tree_root_id)
                .expect("PDF/UA-1 fixture structure tree root")
                .as_dict()
                .expect("PDF/UA-1 fixture structure tree root dictionary")
                .get(b"K")
                .expect("PDF/UA-1 fixture structure tree root kids")
                .as_array()
                .expect("PDF/UA-1 fixture structure tree root kids array")
                .last()
                .expect("PDF/UA-1 fixture table")
                .as_reference()
                .expect("indirect PDF/UA-1 fixture table");
            let row_ids = document
                .get_object(table_id)
                .expect("PDF/UA-1 fixture table")
                .as_dict()
                .expect("PDF/UA-1 fixture table dictionary")
                .get(b"K")
                .expect("PDF/UA-1 fixture table kids")
                .as_array()
                .expect("PDF/UA-1 fixture table kids array")
                .iter()
                .filter_map(|kid| {
                    let kid_id = kid.as_reference().ok()?;
                    let dictionary = document.get_object(kid_id).ok()?.as_dict().ok()?;
                    let structure_type = dictionary.get(b"S").ok()?.as_name().ok()?;
                    match structure_type {
                        b"TR" => Some(kid_id),
                        b"THead" | b"TBody" | b"TFoot" => dictionary
                            .get(b"K")
                            .ok()?
                            .as_array()
                            .ok()?
                            .first()?
                            .as_reference()
                            .ok(),
                        _ => None,
                    }
                })
                .collect::<Vec<_>>();
            for row_id in row_ids {
                let cell_id = document.add_object(dictionary! {
                    "S" => "TD",
                    "P" => Object::Reference(row_id),
                });
                document
                    .get_object_mut(row_id)
                    .expect("PDF/UA-1 fixture row")
                    .as_dict_mut()
                    .expect("PDF/UA-1 fixture row dictionary")
                    .set("K", Object::Reference(cell_id));
            }
        }
        "not_contained" => {
            let cell_id = document.add_object(dictionary! {
                "S" => "TD",
                "P" => Object::Reference(struct_tree_root_id),
            });
            document
                .get_object_mut(struct_tree_root_id)
                .expect("PDF/UA-1 fixture structure tree root")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture structure tree root dictionary")
                .get_mut(b"K")
                .expect("PDF/UA-1 fixture structure tree root kids")
                .as_array_mut()
                .expect("PDF/UA-1 fixture structure tree root kids array")
                .push(Object::Reference(cell_id));
        }
        _ => panic!("unknown PDF/UA-1 rule 7.2-9 fixture case {case}"),
    }

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-9 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_10_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_2_9_fixture("contained"))
        .expect("load PDF/UA-1 TR children fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");

    match case {
        "allowed" => {}
        "invalid" => {
            let table_id = document
                .get_object(struct_tree_root_id)
                .expect("PDF/UA-1 fixture structure tree root")
                .as_dict()
                .expect("PDF/UA-1 fixture structure tree root dictionary")
                .get(b"K")
                .expect("PDF/UA-1 fixture structure tree root kids")
                .as_array()
                .expect("PDF/UA-1 fixture structure tree root kids array")
                .last()
                .expect("PDF/UA-1 fixture table")
                .as_reference()
                .expect("indirect PDF/UA-1 fixture table");
            let row_id = document
                .get_object(table_id)
                .expect("PDF/UA-1 fixture table")
                .as_dict()
                .expect("PDF/UA-1 fixture table dictionary")
                .get(b"K")
                .expect("PDF/UA-1 fixture table kids")
                .as_array()
                .expect("PDF/UA-1 fixture table kids array")
                .iter()
                .find_map(|kid| {
                    let kid_id = kid.as_reference().ok()?;
                    let structure_type = document
                        .get_object(kid_id)
                        .ok()?
                        .as_dict()
                        .ok()?
                        .get(b"S")
                        .ok()?
                        .as_name()
                        .ok()?;
                    (structure_type == b"TR").then_some(kid_id)
                })
                .expect("PDF/UA-1 fixture table row");
            let invalid_child_id = document.add_object(dictionary! {
                "S" => "P",
                "P" => Object::Reference(row_id),
            });
            let existing_kid = document
                .get_object(row_id)
                .expect("PDF/UA-1 fixture table row")
                .as_dict()
                .expect("PDF/UA-1 fixture table row dictionary")
                .get(b"K")
                .expect("PDF/UA-1 fixture table row kids")
                .clone();
            document
                .get_object_mut(row_id)
                .expect("PDF/UA-1 fixture table row")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture table row dictionary")
                .set("K", vec![existing_kid, Object::Reference(invalid_child_id)]);
        }
        _ => panic!("unknown PDF/UA-1 rule 7.2-10 fixture case {case}"),
    }

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-10 fixture");
    bytes
}

pub fn pdfua1_rule_7_3_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 Figure alternative-text fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let mut figure = dictionary! {
        "S" => "Figure",
        "P" => Object::Reference(struct_tree_root_id),
    };
    match case {
        "alt_present" => {
            figure.set("Alt", Object::string_literal("A mountain"));
        }
        "alt_empty" => {
            figure.set("Alt", Object::string_literal(""));
        }
        "actual_text_present" => {
            figure.set("ActualText", Object::string_literal("A mountain"));
        }
        "missing" => {}
        _ => panic!("unknown PDF/UA-1 rule 7.3-1 fixture case {case}"),
    }
    let figure_id = document.add_object(figure);
    document
        .get_object_mut(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get_mut(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array_mut()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .push(Object::Reference(figure_id));
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.3-1 fixture");
    bytes
}

pub fn pdfua1_rule_7_7_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 Formula alternative-text fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let mut formula = dictionary! {
        "S" => "Formula",
        "P" => Object::Reference(struct_tree_root_id),
    };
    match case {
        "alt_present" => {
            formula.set("Alt", Object::string_literal("x squared"));
        }
        "alt_empty" => {
            formula.set("Alt", Object::string_literal(""));
        }
        "actual_text_present" => {
            formula.set("ActualText", Object::string_literal("x²"));
        }
        "missing" => {}
        _ => panic!("unknown PDF/UA-1 rule 7.7-1 fixture case {case}"),
    }
    let formula_id = document.add_object(formula);
    document
        .get_object_mut(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get_mut(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array_mut()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .push(Object::Reference(formula_id));
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.7-1 fixture");
    bytes
}

pub fn pdfua1_rule_7_9_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 Note ID fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let mut note = dictionary! {
        "S" => "Note",
        "P" => Object::Reference(struct_tree_root_id),
    };
    match case {
        "present" => {
            note.set("ID", Object::string_literal("note-1"));
        }
        "missing" => {}
        _ => panic!("unknown PDF/UA-1 rule 7.9-1 fixture case {case}"),
    }
    let note_id = document.add_object(note);
    document
        .get_object_mut(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get_mut(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array_mut()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .push(Object::Reference(note_id));
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.9-1 fixture");
    bytes
}

pub fn pdfua1_rule_7_9_2_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 duplicate Note ID fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let ids = match case {
        "unique" => ["note-1", "note-2"],
        "duplicate" => ["note-1", "note-1"],
        _ => panic!("unknown PDF/UA-1 rule 7.9-2 fixture case {case}"),
    };
    let note_ids = ids
        .into_iter()
        .map(|id| {
            document.add_object(dictionary! {
                "S" => "Note",
                "P" => Object::Reference(struct_tree_root_id),
                "ID" => Object::string_literal(id),
            })
        })
        .collect::<Vec<_>>();
    document
        .get_object_mut(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get_mut(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array_mut()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .extend(note_ids.into_iter().map(Object::Reference));
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.9-2 fixture");
    bytes
}

pub fn pdfua1_rule_7_10_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 optional-content configuration fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let catalog = document
        .get_object_mut(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture catalog dictionary");

    let default = match case {
        "valid" | "missing_config_name" => dictionary! {
            "Name" => Object::string_literal("Default configuration"),
        },
        "missing_default_name" => Dictionary::new(),
        _ => panic!("unknown PDF/UA-1 rule 7.10-1 fixture case {case}"),
    };
    let config = match case {
        "valid" | "missing_default_name" => dictionary! {
            "Name" => Object::string_literal("Alternate configuration"),
        },
        "missing_config_name" => Dictionary::new(),
        _ => panic!("unknown PDF/UA-1 rule 7.10-1 fixture case {case}"),
    };
    catalog.set(
        "OCProperties",
        dictionary! {
            "D" => default,
            "Configs" => vec![Object::Dictionary(config)],
        },
    );

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.10-1 fixture");
    bytes
}

pub fn pdfua1_rule_7_10_2_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 optional-content configuration fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let catalog = document
        .get_object_mut(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture catalog dictionary");

    let mut alternate = dictionary! {
        "Name" => Object::string_literal("Alternate configuration"),
    };
    if case == "as_present" {
        alternate.set("AS", Vec::<Object>::new());
    } else if case != "valid" {
        panic!("unknown PDF/UA-1 rule 7.10-2 fixture case {case}");
    }
    catalog.set(
        "OCProperties",
        dictionary! {
            "D" => dictionary! {
                "Name" => Object::string_literal("Default configuration"),
            },
            "Configs" => vec![Object::Dictionary(alternate)],
        },
    );

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.10-2 fixture");
    bytes
}

pub fn pdfua1_rule_7_11_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 embedded-file specification fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let embedded = document.add_object(Stream::new(
        dictionary! {
            "Type" => "EmbeddedFile",
            "Subtype" => "text/plain",
        },
        b"embedded data".to_vec(),
    ));
    let file_spec = document.add_object(dictionary! {
        "Type" => "Filespec",
        "F" => Object::string_literal("attachment.txt"),
        "UF" => if case == "valid" {
            Object::string_literal("attachment.txt")
        } else if case == "empty_uf" {
            Object::string_literal("")
        } else {
            panic!("unknown PDF/UA-1 rule 7.11-1 fixture case {case}")
        },
        "EF" => dictionary! { "F" => embedded },
    });
    let catalog = document
        .get_object_mut(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture catalog dictionary");
    catalog.set(
        "Names",
        dictionary! {
            "EmbeddedFiles" => dictionary! {
                "Names" => vec![
                    Object::string_literal("attachment"),
                    Object::Reference(file_spec),
                ],
            },
        },
    );

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.11-1 fixture");
    bytes
}

pub fn pdfua1_rule_7_15_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_5_1_fixture("identification_present"))
        .expect("load PDF/UA-1 dynamic-XFA fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let xfa_stream = match case {
        "no_xfa" => None,
        "static_xfa" => Some(
            br#"<config><acrobat><acrobat7><dynamicRender>requiredForDynamicForms</dynamicRender></acrobat7></acrobat></config>"#
                .as_slice(),
        ),
        "dynamic_xfa" => Some(
            br#"<config><acrobat><acrobat7><dynamicRender>required</dynamicRender></acrobat7></acrobat></config>"#
                .as_slice(),
        ),
        _ => panic!("unknown PDF/UA-1 rule 7.15-1 fixture case {case}"),
    };
    let acro_form = match xfa_stream {
        None => dictionary! {},
        Some(xfa_stream) => dictionary! {
            "XFA" => document.add_object(Stream::new(Dictionary::new(), xfa_stream.to_vec())),
        },
    };
    let acro_form_id = document.add_object(acro_form);
    document
        .get_object_mut(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .set("AcroForm", acro_form_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 dynamic-XFA fixture");
    bytes
}

pub fn pdfua1_rule_7_16_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_5_1_fixture("identification_present"))
        .expect("load PDF/UA-1 encryption fixture");
    let permissions = match case {
        "valid" => Permissions::all(),
        "bit_10_false" | "missing_p" => Permissions::empty(),
        _ => panic!("unknown PDF/UA-1 rule 7.16-1 fixture case {case}"),
    };
    let state = EncryptionState::try_from(EncryptionVersion::V1 {
        document: &document,
        owner_password: "owner",
        user_password: "",
        permissions,
    })
    .expect("create PDF/UA-1 encryption state");
    document
        .encrypt(&state)
        .expect("encrypt PDF/UA-1 rule 7.16-1 fixture");
    if case == "missing_p" {
        let encryption_id = document
            .trailer
            .get(b"Encrypt")
            .expect("PDF/UA-1 encryption dictionary reference")
            .as_reference()
            .expect("indirect PDF/UA-1 encryption dictionary");
        document
            .get_object_mut(encryption_id)
            .expect("PDF/UA-1 encryption dictionary")
            .as_dict_mut()
            .expect("PDF/UA-1 encryption dictionary")
            .remove(b"P");
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.16-1 fixture");
    bytes
}

pub fn pdfua1_rule_7_4_2_1_fixture(case: &str) -> Vec<u8> {
    let heading_levels = match case {
        "valid" => vec![1, 1, 2, 3, 3, 4, 2, 3, 1],
        "first_heading_h2" => vec![2],
        "skips_h2" => vec![1, 3],
        _ => panic!("unknown PDF/UA-1 rule 7.4.2-1 fixture case {case}"),
    };
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 heading-nesting fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let heading_ids = heading_levels
        .into_iter()
        .map(|level| {
            document.add_object(dictionary! {
                "S" => format!("H{level}"),
                "P" => Object::Reference(struct_tree_root_id),
            })
        })
        .collect::<Vec<_>>();
    let heading_objects = heading_ids
        .into_iter()
        .map(Object::Reference)
        .collect::<Vec<_>>();
    let kids = document
        .get_object_mut(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get_mut(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array_mut()
        .expect("PDF/UA-1 fixture structure tree root kids array");
    if case == "first_heading_h2" {
        kids.splice(0..0, heading_objects);
    } else {
        kids.extend(heading_objects);
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 heading-nesting fixture");
    bytes
}

fn pdfua1_heading_fixture(heading_types: &[&str], description: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_5_1_fixture("identification_present"))
        .expect("load PDF/UA-1 heading fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document.add_object(dictionary! {
        "Type" => "StructTreeRoot",
        "K" => Vec::<Object>::new(),
    });
    let catalog = document
        .get_object_mut(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture catalog dictionary");
    catalog.set("Lang", Object::string_literal("en"));
    catalog.set("StructTreeRoot", Object::Reference(struct_tree_root_id));
    let parent_id = document.new_object_id();
    let heading_ids = heading_types
        .iter()
        .map(|heading_type| {
            document.add_object(dictionary! {
                "S" => *heading_type,
                "P" => Object::Reference(parent_id),
            })
        })
        .map(Object::Reference)
        .collect::<Vec<_>>();
    document.objects.insert(
        parent_id,
        dictionary! {
            "S" => "Div",
            "P" => Object::Reference(struct_tree_root_id),
            "K" => heading_ids,
        }
        .into(),
    );
    document
        .get_object_mut(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get_mut(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array_mut()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .push(Object::Reference(parent_id));
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).expect(description);
    bytes
}

pub fn pdfua1_rule_7_4_4_1_fixture(case: &str) -> Vec<u8> {
    let heading_types: &[&str] = match case {
        "single_h" => &["H"],
        "multiple_h" => &["H", "H"],
        _ => panic!("unknown PDF/UA-1 rule 7.4.4-1 fixture case {case}"),
    };
    pdfua1_heading_fixture(heading_types, "save PDF/UA-1 heading-child-count fixture")
}

pub fn pdfua1_rule_7_4_4_2_fixture(case: &str) -> Vec<u8> {
    let heading_types: &[&str] = match case {
        "h_only" => &["H"],
        "hn_only" => &["H1"],
        "h_then_hn" => &["H", "H1"],
        "hn_then_h" => &["H1", "H"],
        _ => panic!("unknown PDF/UA-1 rule 7.4.4-2 fixture case {case}"),
    };
    pdfua1_heading_fixture(heading_types, "save PDF/UA-1 heading-structure fixture")
}

pub fn pdfua1_rule_7_2_36_fixture(case: &str) -> Vec<u8> {
    pdfua1_table_section_fixture(case, "THead", "7.2-36")
}

pub fn pdfua1_rule_7_2_37_fixture(case: &str) -> Vec<u8> {
    pdfua1_table_section_fixture(case, "TBody", "7.2-37")
}

pub fn pdfua1_rule_7_2_38_fixture(case: &str) -> Vec<u8> {
    pdfua1_table_section_fixture(case, "TFoot", "7.2-38")
}

pub fn pdfua1_rule_7_2_39_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_2_3_fixture("allowed"))
        .expect("load PDF/UA-1 table Caption fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let table_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .last()
        .expect("PDF/UA-1 fixture table")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture table");

    if case == "invalid" {
        let caption_id = document.add_object(dictionary! {
            "S" => "Caption",
            "P" => Object::Reference(table_id),
        });
        document
            .get_object_mut(table_id)
            .expect("PDF/UA-1 fixture table")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture table dictionary")
            .get_mut(b"K")
            .expect("PDF/UA-1 fixture table kids")
            .as_array_mut()
            .expect("PDF/UA-1 fixture table kids array")
            .insert(0, Object::Reference(caption_id));
    } else if case != "allowed" {
        panic!("unknown PDF/UA-1 rule 7.2-39 fixture case {case}");
    }

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-39 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_16_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_2_3_fixture("allowed"))
        .expect("load PDF/UA-1 table Caption-position fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let table_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .last()
        .expect("PDF/UA-1 fixture table")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture table");
    let position = match case {
        "caption_first" => 0,
        "caption_last" => usize::MAX,
        "caption_middle" => 2,
        _ => panic!("unknown PDF/UA-1 rule 7.2-16 fixture case {case}"),
    };
    let caption_id = document
        .get_object(table_id)
        .expect("PDF/UA-1 fixture table")
        .as_dict()
        .expect("PDF/UA-1 fixture table dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture table kids")
        .as_array()
        .expect("PDF/UA-1 fixture table kids array")
        .iter()
        .find_map(|kid| {
            let kid_id = kid.as_reference().ok()?;
            let structure_type = document
                .get_object(kid_id)
                .ok()?
                .as_dict()
                .ok()?
                .get(b"S")
                .ok()?
                .as_name()
                .ok()?;
            (structure_type == b"Caption").then_some(kid_id)
        })
        .expect("PDF/UA-1 fixture Caption");
    let table = document
        .get_object_mut(table_id)
        .expect("PDF/UA-1 fixture table")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture table dictionary");
    let kids = table
        .get_mut(b"K")
        .expect("PDF/UA-1 fixture table kids")
        .as_array_mut()
        .expect("PDF/UA-1 fixture table kids array");
    let caption_index = kids
        .iter()
        .position(|kid| kid.as_reference().ok() == Some(caption_id))
        .expect("PDF/UA-1 fixture Caption");
    let caption = kids.remove(caption_index);
    let insertion_index = if position == usize::MAX {
        kids.len()
    } else {
        position
    };
    kids.insert(insertion_index, caption);

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-16 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_11_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_2_3_fixture("allowed"))
        .expect("load PDF/UA-1 table THead fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let table_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .last()
        .expect("PDF/UA-1 fixture table")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture table");

    if case == "invalid" {
        let thead_id = document.add_object(dictionary! {
            "S" => "THead",
            "P" => Object::Reference(table_id),
        });
        document
            .get_object_mut(table_id)
            .expect("PDF/UA-1 fixture table")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture table dictionary")
            .get_mut(b"K")
            .expect("PDF/UA-1 fixture table kids")
            .as_array_mut()
            .expect("PDF/UA-1 fixture table kids array")
            .insert(0, Object::Reference(thead_id));
    } else if case != "allowed" {
        panic!("unknown PDF/UA-1 rule 7.2-11 fixture case {case}");
    }

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-11 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_12_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_2_3_fixture("allowed"))
        .expect("load PDF/UA-1 table TFoot fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let table_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .last()
        .expect("PDF/UA-1 fixture table")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture table");

    if case == "invalid" {
        let tfoot_id = document.add_object(dictionary! {
            "S" => "TFoot",
            "P" => Object::Reference(table_id),
        });
        document
            .get_object_mut(table_id)
            .expect("PDF/UA-1 fixture table")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture table dictionary")
            .get_mut(b"K")
            .expect("PDF/UA-1 fixture table kids")
            .as_array_mut()
            .expect("PDF/UA-1 fixture table kids array")
            .insert(0, Object::Reference(tfoot_id));
    } else if case != "allowed" {
        panic!("unknown PDF/UA-1 rule 7.2-12 fixture case {case}");
    }

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-12 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_13_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_2_3_fixture("allowed"))
        .expect("load PDF/UA-1 table TFoot/TBody fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let table_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .last()
        .expect("PDF/UA-1 fixture table")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture table");

    if case == "invalid" {
        let section_ids = document
            .get_object(table_id)
            .expect("PDF/UA-1 fixture table")
            .as_dict()
            .expect("PDF/UA-1 fixture table dictionary")
            .get(b"K")
            .expect("PDF/UA-1 fixture table kids")
            .as_array()
            .expect("PDF/UA-1 fixture table kids array")
            .iter()
            .filter_map(|kid| {
                let kid_id = kid.as_reference().ok()?;
                let structure_type = document
                    .get_object(kid_id)
                    .ok()?
                    .as_dict()
                    .ok()?
                    .get(b"S")
                    .ok()?
                    .as_name()
                    .ok()?;
                matches!(structure_type, b"THead" | b"TBody").then_some(kid_id)
            })
            .collect::<Vec<_>>();
        document
            .get_object_mut(table_id)
            .expect("PDF/UA-1 fixture table")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture table dictionary")
            .get_mut(b"K")
            .expect("PDF/UA-1 fixture table kids")
            .as_array_mut()
            .expect("PDF/UA-1 fixture table kids array")
            .retain(|kid| {
                kid.as_reference()
                    .map(|kid_id| !section_ids.contains(&kid_id))
                    .unwrap_or(true)
            });
    } else if case != "allowed" {
        panic!("unknown PDF/UA-1 rule 7.2-13 fixture case {case}");
    }

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-13 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_14_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_2_3_fixture("allowed"))
        .expect("load PDF/UA-1 table THead/TBody fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let table_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .last()
        .expect("PDF/UA-1 fixture table")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture table");

    if case == "invalid" {
        let section_ids = document
            .get_object(table_id)
            .expect("PDF/UA-1 fixture table")
            .as_dict()
            .expect("PDF/UA-1 fixture table dictionary")
            .get(b"K")
            .expect("PDF/UA-1 fixture table kids")
            .as_array()
            .expect("PDF/UA-1 fixture table kids array")
            .iter()
            .filter_map(|kid| {
                let kid_id = kid.as_reference().ok()?;
                let structure_type = document
                    .get_object(kid_id)
                    .ok()?
                    .as_dict()
                    .ok()?
                    .get(b"S")
                    .ok()?
                    .as_name()
                    .ok()?;
                matches!(structure_type, b"TBody" | b"TFoot").then_some(kid_id)
            })
            .collect::<Vec<_>>();
        document
            .get_object_mut(table_id)
            .expect("PDF/UA-1 fixture table")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture table dictionary")
            .get_mut(b"K")
            .expect("PDF/UA-1 fixture table kids")
            .as_array_mut()
            .expect("PDF/UA-1 fixture table kids array")
            .retain(|kid| {
                kid.as_reference()
                    .map(|kid_id| !section_ids.contains(&kid_id))
                    .unwrap_or(true)
            });
    } else if case != "allowed" {
        panic!("unknown PDF/UA-1 rule 7.2-14 fixture case {case}");
    }

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-14 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_41_fixture(case: &str) -> Vec<u8> {
    let row_spans = match case {
        "allowed" => [[2, 2], [0, 0]].as_slice(),
        "invalid" => [[2, 3], [0, 0]].as_slice(),
        _ => panic!("unknown PDF/UA-1 rule 7.2-41 fixture case {case}"),
    };
    let mut document = Document::load_mem(&pdfua1_rule_7_2_3_fixture("allowed"))
        .expect("load PDF/UA-1 table row-span fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let table_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .last()
        .expect("PDF/UA-1 fixture table")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture table");
    let tbody_id = document
        .get_object(table_id)
        .expect("PDF/UA-1 fixture table")
        .as_dict()
        .expect("PDF/UA-1 fixture table dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture table kids")
        .as_array()
        .expect("PDF/UA-1 fixture table kids array")
        .iter()
        .find_map(|kid| {
            let kid_id = kid.as_reference().ok()?;
            let structure_type = document
                .get_object(kid_id)
                .ok()?
                .as_dict()
                .ok()?
                .get(b"S")
                .ok()?
                .as_name()
                .ok()?;
            (structure_type == b"TBody").then_some(kid_id)
        })
        .expect("PDF/UA-1 fixture TBody");

    let mut row_ids = Vec::with_capacity(row_spans.len());
    for spans in row_spans {
        let row_id = document.add_object(dictionary! {
            "S" => "TR",
            "P" => Object::Reference(tbody_id),
        });
        let cells = spans
            .iter()
            .copied()
            .filter(|row_span| *row_span > 0)
            .map(|row_span| {
                document.add_object(dictionary! {
                    "S" => "TD",
                    "P" => Object::Reference(row_id),
                    "A" => dictionary! {
                        "O" => "Table",
                        "RowSpan" => row_span,
                    },
                })
            })
            .collect::<Vec<_>>();
        document
            .get_object_mut(row_id)
            .expect("PDF/UA-1 fixture row")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture row dictionary")
            .set(
                "K",
                cells.into_iter().map(Object::Reference).collect::<Vec<_>>(),
            );
        row_ids.push(row_id);
    }
    document
        .get_object_mut(tbody_id)
        .expect("PDF/UA-1 fixture TBody")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture TBody dictionary")
        .set(
            "K",
            row_ids
                .into_iter()
                .map(Object::Reference)
                .collect::<Vec<_>>(),
        );
    document
        .get_object_mut(table_id)
        .expect("PDF/UA-1 fixture table")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture table dictionary")
        .set("K", vec![Object::Reference(tbody_id)]);

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-41 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_42_fixture(case: &str) -> Vec<u8> {
    let column_spans = match case {
        "allowed" => vec![vec![2_i64], vec![1, 1]],
        "invalid" => vec![vec![2_i64], vec![1, 1, 1]],
        _ => panic!("unknown PDF/UA-1 rule 7.2-42 fixture case {case}"),
    };
    let mut document = Document::load_mem(&pdfua1_rule_7_2_3_fixture("allowed"))
        .expect("load PDF/UA-1 fixture for rule 7.2-42");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let table_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .last()
        .expect("PDF/UA-1 fixture table")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture table");
    let tbody_id = document
        .get_object(table_id)
        .expect("PDF/UA-1 fixture table")
        .as_dict()
        .expect("PDF/UA-1 fixture table dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture table kids")
        .as_array()
        .expect("PDF/UA-1 fixture table kids array")
        .iter()
        .find_map(|kid| {
            let kid_id = kid.as_reference().ok()?;
            let structure_type = document
                .get_object(kid_id)
                .ok()?
                .as_dict()
                .ok()?
                .get(b"S")
                .ok()?
                .as_name()
                .ok()?;
            (structure_type == b"TBody").then_some(kid_id)
        })
        .expect("PDF/UA-1 fixture TBody");

    let row_ids = column_spans
        .into_iter()
        .map(|spans| {
            let row_id = document.add_object(dictionary! {
                "S" => "TR",
                "P" => Object::Reference(tbody_id),
            });
            let cell_ids = spans
                .into_iter()
                .map(|column_span| {
                    document.add_object(dictionary! {
                        "S" => "TD",
                        "P" => Object::Reference(row_id),
                        "A" => dictionary! {
                            "O" => "Table",
                            "ColSpan" => column_span,
                        },
                    })
                })
                .collect::<Vec<_>>();
            document
                .get_object_mut(row_id)
                .expect("PDF/UA-1 fixture row")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture row dictionary")
                .set(
                    "K",
                    cell_ids
                        .into_iter()
                        .map(Object::Reference)
                        .collect::<Vec<_>>(),
                );
            row_id
        })
        .collect::<Vec<_>>();
    document
        .get_object_mut(tbody_id)
        .expect("PDF/UA-1 fixture TBody")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture TBody dictionary")
        .set(
            "K",
            row_ids
                .into_iter()
                .map(Object::Reference)
                .collect::<Vec<_>>(),
        );
    document
        .get_object_mut(table_id)
        .expect("PDF/UA-1 fixture table")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture table dictionary")
        .set("K", vec![Object::Reference(tbody_id)]);

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-42 fixture");
    bytes
}

pub fn pdfua1_rule_7_2_15_fixture(case: &str) -> Vec<u8> {
    let cell_spans = match case {
        "allowed" => vec![vec![(1_i64, 1_i64), (1, 1)], vec![(1, 1), (1, 1)]],
        "invalid" => vec![
            vec![(2_i64, 1_i64), (1, 1), (2, 1), (1, 1), (1, 1)],
            vec![(1, 2)],
            vec![(1, 1), (1, 1), (1, 1), (1, 1), (1, 1)],
        ],
        _ => panic!("unknown PDF/UA-1 rule 7.2-15 fixture case {case}"),
    };
    let mut document = Document::load_mem(&pdfua1_rule_7_2_3_fixture("allowed"))
        .expect("load PDF/UA-1 table intersection fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let table_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .last()
        .expect("PDF/UA-1 fixture table")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture table");
    let tbody_id = document
        .get_object(table_id)
        .expect("PDF/UA-1 fixture table")
        .as_dict()
        .expect("PDF/UA-1 fixture table dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture table kids")
        .as_array()
        .expect("PDF/UA-1 fixture table kids array")
        .iter()
        .find_map(|kid| {
            let kid_id = kid.as_reference().ok()?;
            let structure_type = document
                .get_object(kid_id)
                .ok()?
                .as_dict()
                .ok()?
                .get(b"S")
                .ok()?
                .as_name()
                .ok()?;
            (structure_type == b"TBody").then_some(kid_id)
        })
        .expect("PDF/UA-1 fixture TBody");
    let row_ids = cell_spans
        .into_iter()
        .map(|spans| {
            let row_id = document.add_object(dictionary! {
                "S" => "TR",
                "P" => Object::Reference(tbody_id),
            });
            let cell_ids = spans
                .into_iter()
                .map(|(row_span, column_span)| {
                    document.add_object(dictionary! {
                        "S" => "TD",
                        "P" => Object::Reference(row_id),
                        "A" => dictionary! {
                            "O" => "Table",
                            "RowSpan" => row_span,
                            "ColSpan" => column_span,
                        },
                    })
                })
                .collect::<Vec<_>>();
            document
                .get_object_mut(row_id)
                .expect("PDF/UA-1 fixture row")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture row dictionary")
                .set(
                    "K",
                    cell_ids
                        .into_iter()
                        .map(Object::Reference)
                        .collect::<Vec<_>>(),
                );
            row_id
        })
        .collect::<Vec<_>>();
    document
        .get_object_mut(tbody_id)
        .expect("PDF/UA-1 fixture TBody")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture TBody dictionary")
        .set(
            "K",
            row_ids
                .into_iter()
                .map(Object::Reference)
                .collect::<Vec<_>>(),
        );
    document
        .get_object_mut(table_id)
        .expect("PDF/UA-1 fixture table")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture table dictionary")
        .set("K", vec![Object::Reference(tbody_id)]);

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.2-15 fixture");
    bytes
}

pub fn pdfua1_rule_7_5_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_2_3_fixture("allowed"))
        .expect("load PDF/UA-1 table header-scope fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let table_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .last()
        .expect("PDF/UA-1 fixture table")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture table");
    let tbody_id = document
        .get_object(table_id)
        .expect("PDF/UA-1 fixture table")
        .as_dict()
        .expect("PDF/UA-1 fixture table dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture table kids")
        .as_array()
        .expect("PDF/UA-1 fixture table kids array")
        .iter()
        .find_map(|kid| {
            let kid_id = kid.as_reference().ok()?;
            let structure_type = document
                .get_object(kid_id)
                .ok()?
                .as_dict()
                .ok()?
                .get(b"S")
                .ok()?
                .as_name()
                .ok()?;
            (structure_type == b"TBody").then_some(kid_id)
        })
        .expect("PDF/UA-1 fixture TBody");
    let scope = match case {
        "scope_present" => Some("Column"),
        "scope_missing" => None,
        _ => panic!("unknown PDF/UA-1 rule 7.5-1 fixture case {case}"),
    };
    let first_row_id = document.add_object(dictionary! {
        "S" => "TR",
        "P" => Object::Reference(tbody_id),
    });
    let mut header = dictionary! {
        "S" => "TH",
        "P" => Object::Reference(first_row_id),
    };
    if let Some(scope) = scope {
        header.set(
            "A",
            dictionary! {
                "O" => "Table",
                "Scope" => scope,
            },
        );
    }
    let header_id = document.add_object(header);
    document
        .get_object_mut(first_row_id)
        .expect("PDF/UA-1 fixture first row")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture first row dictionary")
        .set("K", vec![Object::Reference(header_id)]);

    let second_row_id = document.add_object(dictionary! {
        "S" => "TR",
        "P" => Object::Reference(tbody_id),
    });
    let data_id = document.add_object(dictionary! {
        "S" => "TD",
        "P" => Object::Reference(second_row_id),
    });
    document
        .get_object_mut(second_row_id)
        .expect("PDF/UA-1 fixture second row")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture second row dictionary")
        .set("K", vec![Object::Reference(data_id)]);
    document
        .get_object_mut(tbody_id)
        .expect("PDF/UA-1 fixture TBody")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture TBody dictionary")
        .set(
            "K",
            vec![
                Object::Reference(first_row_id),
                Object::Reference(second_row_id),
            ],
        );
    document
        .get_object_mut(table_id)
        .expect("PDF/UA-1 fixture table")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture table dictionary")
        .set("K", vec![Object::Reference(tbody_id)]);

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.5-1 fixture");
    bytes
}

pub fn pdfua1_rule_7_5_2_fixture(case: &str) -> Vec<u8> {
    let scope_case = match case {
        "scope_present" => "scope_present",
        "scope_missing" => "scope_missing",
        _ => panic!("unknown PDF/UA-1 rule 7.5-2 fixture case {case}"),
    };
    let mut document = Document::load_mem(&pdfua1_rule_7_5_1_fixture(scope_case))
        .expect("load PDF/UA-1 undefined-header fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let table_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .last()
        .expect("PDF/UA-1 fixture table")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture table");
    let tbody_id = document
        .get_object(table_id)
        .expect("PDF/UA-1 fixture table")
        .as_dict()
        .expect("PDF/UA-1 fixture table dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture table kids")
        .as_array()
        .expect("PDF/UA-1 fixture table kids array")
        .first()
        .expect("PDF/UA-1 fixture TBody")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture TBody");
    let data_row_id = document
        .get_object(tbody_id)
        .expect("PDF/UA-1 fixture TBody")
        .as_dict()
        .expect("PDF/UA-1 fixture TBody dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture TBody kids")
        .as_array()
        .expect("PDF/UA-1 fixture TBody kids array")
        .last()
        .expect("PDF/UA-1 fixture data row")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture data row");
    let data_id = document
        .get_object(data_row_id)
        .expect("PDF/UA-1 fixture data row")
        .as_dict()
        .expect("PDF/UA-1 fixture data row dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture data row kids")
        .as_array()
        .expect("PDF/UA-1 fixture data row kids array")
        .first()
        .expect("PDF/UA-1 fixture data cell")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture data cell");
    document
        .get_object_mut(data_id)
        .expect("PDF/UA-1 fixture data cell")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture data cell dictionary")
        .set(
            "A",
            dictionary! {
                "O" => "Table",
                "Headers" => vec![Object::String(
                    b"missing-header".to_vec(),
                    StringFormat::Literal,
                )],
            },
        );

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.5-2 fixture");
    bytes
}

fn pdfua1_table_section_fixture(case: &str, section_type: &str, rule: &str) -> Vec<u8> {
    let child_type = match case {
        "allowed" => "TR",
        "invalid" => "P",
        _ => panic!("unknown PDF/UA-1 rule {rule} fixture case {case}"),
    };
    let mut document = Document::load_mem(&pdfua1_rule_7_2_3_fixture("allowed"))
        .expect("load PDF/UA-1 table section children fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let table_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .last()
        .expect("PDF/UA-1 fixture table")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture table");
    let section_id = {
        let table = document
            .get_object(table_id)
            .expect("PDF/UA-1 fixture table")
            .as_dict()
            .expect("PDF/UA-1 fixture table dictionary");
        table
            .get(b"K")
            .expect("PDF/UA-1 fixture table kids")
            .as_array()
            .expect("PDF/UA-1 fixture table kids array")
            .iter()
            .find_map(|kid| {
                let kid_id = kid.as_reference().ok()?;
                let structure_type = document
                    .get_object(kid_id)
                    .ok()?
                    .as_dict()
                    .ok()?
                    .get(b"S")
                    .ok()?
                    .as_name()
                    .ok()?;
                (structure_type == section_type.as_bytes()).then_some(kid_id)
            })
            .expect("PDF/UA-1 fixture table section")
    };
    let child_id = document.add_object(dictionary! {
        "S" => child_type,
        "P" => Object::Reference(section_id),
    });
    document
        .get_object_mut(section_id)
        .expect("PDF/UA-1 fixture table section")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture table section dictionary")
        .set("K", vec![Object::Reference(child_id)]);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 table section fixture");
    bytes
}

pub fn pdfua1_rule_7_18_1_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 annotation-tag fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let (page_id, struct_tree_root_id) = {
        let catalog = document
            .get_object(root_id)
            .expect("PDF/UA-1 fixture catalog")
            .as_dict()
            .expect("PDF/UA-1 fixture catalog dictionary");
        let pages_id = catalog
            .get(b"Pages")
            .expect("PDF/UA-1 fixture pages")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture pages");
        let page_id = document
            .get_object(pages_id)
            .expect("PDF/UA-1 fixture pages object")
            .as_dict()
            .expect("PDF/UA-1 fixture pages dictionary")
            .get(b"Kids")
            .expect("PDF/UA-1 fixture page kids")
            .as_array()
            .expect("PDF/UA-1 fixture page kids array")
            .first()
            .expect("PDF/UA-1 fixture page")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture page");
        let struct_tree_root_id = catalog
            .get(b"StructTreeRoot")
            .expect("PDF/UA-1 fixture structure tree root")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture structure tree root");
        (page_id, struct_tree_root_id)
    };
    let mut annotation = dictionary! {
        "Type" => "Annot",
        "Subtype" => "Text",
        "Rect" => vec![10.into(), 10.into(), 20.into(), 20.into()],
        "StructParent" => 100,
    };
    if matches!(case, "valid" | "invalid") {
        annotation.set("Contents", Object::string_literal("Annotation"));
    }
    let annotation_id = document.add_object(annotation);
    if case == "valid" {
        let object_reference_id = document.add_object(dictionary! {
            "Type" => "OBJR",
            "Obj" => annotation_id,
        });
        let structure_annotation_id = document.add_object(dictionary! {
            "Type" => "StructElem",
            "S" => "Annot",
            "P" => struct_tree_root_id,
            "K" => object_reference_id,
            "Pg" => page_id,
            "Lang" => Object::string_literal("en"),
        });
        let structure_tree_root = document
            .get_object_mut(struct_tree_root_id)
            .expect("PDF/UA-1 fixture structure tree root")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture structure tree root dictionary");
        structure_tree_root
            .get_mut(b"K")
            .expect("PDF/UA-1 fixture structure tree root kids")
            .as_array_mut()
            .expect("PDF/UA-1 fixture structure tree root kids array")
            .push(Object::Reference(structure_annotation_id));
        structure_tree_root
            .get_mut(b"ParentTree")
            .expect("PDF/UA-1 fixture parent tree")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture parent tree dictionary")
            .get_mut(b"Nums")
            .expect("PDF/UA-1 fixture parent tree numbers")
            .as_array_mut()
            .expect("PDF/UA-1 fixture parent tree numbers array")
            .extend([
                Object::Integer(100),
                Object::Reference(structure_annotation_id),
            ]);
    } else if case != "invalid" {
        panic!("unknown PDF/UA-1 rule 7.18.1-1 fixture case {case}");
    }
    let page = document
        .get_object_mut(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture page dictionary");
    page.set("Tabs", "S");
    page.set("Annots", vec![Object::Reference(annotation_id)]);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 annotation-tag fixture");
    bytes
}

pub fn pdfua1_rule_7_18_1_2_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_18_1_1_fixture("valid"))
        .expect("load PDF/UA-1 annotation-alt-text fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let (page_id, struct_tree_root_id) = {
        let catalog = document
            .get_object(root_id)
            .expect("PDF/UA-1 fixture catalog")
            .as_dict()
            .expect("PDF/UA-1 fixture catalog dictionary");
        let pages_id = catalog
            .get(b"Pages")
            .expect("PDF/UA-1 fixture pages")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture pages");
        let page_id = document
            .get_object(pages_id)
            .expect("PDF/UA-1 fixture pages object")
            .as_dict()
            .expect("PDF/UA-1 fixture pages dictionary")
            .get(b"Kids")
            .expect("PDF/UA-1 fixture page kids")
            .as_array()
            .expect("PDF/UA-1 fixture page kids array")
            .first()
            .expect("PDF/UA-1 fixture page")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture page");
        let struct_tree_root_id = catalog
            .get(b"StructTreeRoot")
            .expect("PDF/UA-1 fixture structure tree root")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture structure tree root");
        (page_id, struct_tree_root_id)
    };
    let annotation_id = document
        .get_object(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict()
        .expect("PDF/UA-1 fixture page dictionary")
        .get(b"Annots")
        .expect("PDF/UA-1 fixture annotations")
        .as_array()
        .expect("PDF/UA-1 fixture annotations array")
        .first()
        .expect("PDF/UA-1 fixture annotation")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture annotation");
    let structure_annotation_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"ParentTree")
        .expect("PDF/UA-1 fixture parent tree")
        .as_dict()
        .expect("PDF/UA-1 fixture parent tree dictionary")
        .get(b"Nums")
        .expect("PDF/UA-1 fixture parent tree numbers")
        .as_array()
        .expect("PDF/UA-1 fixture parent tree numbers array")
        .chunks(2)
        .find_map(|pair| {
            (pair.first()?.as_i64().ok()? == 100)
                .then(|| pair.get(1)?.as_reference().ok())
                .flatten()
        })
        .expect("PDF/UA-1 fixture annotation structure element");
    match case {
        "contents" => {}
        "alt" => {
            document
                .get_object_mut(annotation_id)
                .expect("PDF/UA-1 fixture annotation")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture annotation dictionary")
                .remove(b"Contents");
            document
                .get_object_mut(structure_annotation_id)
                .expect("PDF/UA-1 fixture annotation structure element")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture annotation structure element dictionary")
                .set("Alt", Object::string_literal("Alternative"));
        }
        "missing" => {
            document
                .get_object_mut(annotation_id)
                .expect("PDF/UA-1 fixture annotation")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture annotation dictionary")
                .remove(b"Contents");
        }
        "empty_contents" => {
            document
                .get_object_mut(annotation_id)
                .expect("PDF/UA-1 fixture annotation")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture annotation dictionary")
                .set("Contents", Object::string_literal(""));
        }
        _ => panic!("unknown PDF/UA-1 rule 7.18.1-2 fixture case {case}"),
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 annotation-alt-text fixture");
    bytes
}

pub fn pdfua1_rule_7_18_1_3_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 form-field alternative-text fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let (page_id, struct_tree_root_id) = {
        let catalog = document
            .get_object(root_id)
            .expect("PDF/UA-1 fixture catalog")
            .as_dict()
            .expect("PDF/UA-1 fixture catalog dictionary");
        let pages_id = catalog
            .get(b"Pages")
            .expect("PDF/UA-1 fixture pages")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture pages");
        let page_id = document
            .get_object(pages_id)
            .expect("PDF/UA-1 fixture pages object")
            .as_dict()
            .expect("PDF/UA-1 fixture pages dictionary")
            .get(b"Kids")
            .expect("PDF/UA-1 fixture page kids")
            .as_array()
            .expect("PDF/UA-1 fixture page kids array")
            .first()
            .expect("PDF/UA-1 fixture page")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture page");
        let struct_tree_root_id = catalog
            .get(b"StructTreeRoot")
            .expect("PDF/UA-1 fixture structure tree root")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture structure tree root");
        (page_id, struct_tree_root_id)
    };
    let appearance_id = document.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 100.into(), 20.into()],
        },
        Vec::new(),
    ));
    let mut widget = dictionary! {
        "Type" => "Annot",
        "Subtype" => "Widget",
        "Rect" => vec![10.into(), 10.into(), 110.into(), 30.into()],
        "F" => 4,
        "FT" => "Tx",
        "T" => Object::string_literal("field"),
        "AP" => dictionary! {"N" => appearance_id},
        "StructParent" => 200,
    };
    match case {
        "tu" => widget.set("TU", Object::string_literal("Field description")),
        "empty_tu" => widget.set("TU", Object::string_literal("")),
        "alt" | "missing" => {}
        _ => panic!("unknown PDF/UA-1 rule 7.18.1-3 fixture case {case}"),
    }
    let widget_id = document.add_object(widget);
    let object_reference_id = document.add_object(dictionary! {
        "Type" => "OBJR",
        "Obj" => widget_id,
    });
    let mut structure_form = dictionary! {
        "Type" => "StructElem",
        "S" => "Form",
        "P" => struct_tree_root_id,
        "K" => object_reference_id,
        "Pg" => page_id,
        "Lang" => Object::string_literal("en"),
    };
    if case == "alt" {
        structure_form.set("Alt", Object::string_literal("Field description"));
    }
    let structure_form_id = document.add_object(structure_form);
    let acro_form_id = document.add_object(dictionary! {
        "Fields" => vec![Object::Reference(widget_id)],
        "NeedAppearances" => false,
    });
    let catalog = document
        .get_object_mut(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture catalog dictionary");
    catalog.set("AcroForm", acro_form_id);
    let structure_tree_root = document
        .get_object_mut(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture structure tree root dictionary");
    structure_tree_root
        .get_mut(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array_mut()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .push(Object::Reference(structure_form_id));
    structure_tree_root
        .get_mut(b"ParentTree")
        .expect("PDF/UA-1 fixture parent tree")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture parent tree dictionary")
        .get_mut(b"Nums")
        .expect("PDF/UA-1 fixture parent tree numbers")
        .as_array_mut()
        .expect("PDF/UA-1 fixture parent tree numbers array")
        .extend([Object::Integer(200), Object::Reference(structure_form_id)]);
    let page = document
        .get_object_mut(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture page dictionary");
    page.set("Tabs", "S");
    page.set("Annots", vec![Object::Reference(widget_id)]);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 form-field alternative-text fixture");
    bytes
}

pub fn pdfua1_rule_7_18_2_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_18_1_1_fixture("valid"))
        .expect("load PDF/UA-1 TrapNet annotation fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let pages_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"Pages")
        .expect("PDF/UA-1 fixture pages")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture pages");
    let page_id = document
        .get_object(pages_id)
        .expect("PDF/UA-1 fixture pages object")
        .as_dict()
        .expect("PDF/UA-1 fixture pages dictionary")
        .get(b"Kids")
        .expect("PDF/UA-1 fixture page kids")
        .as_array()
        .expect("PDF/UA-1 fixture page kids array")
        .first()
        .expect("PDF/UA-1 fixture page")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture page");
    let annotation_id = document
        .get_object(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict()
        .expect("PDF/UA-1 fixture page dictionary")
        .get(b"Annots")
        .expect("PDF/UA-1 fixture annotations")
        .as_array()
        .expect("PDF/UA-1 fixture annotations array")
        .first()
        .expect("PDF/UA-1 fixture annotation")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture annotation");
    match case {
        "allowed" => {}
        "forbidden" => {
            document
                .get_object_mut(annotation_id)
                .expect("PDF/UA-1 fixture annotation")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture annotation dictionary")
                .set("Subtype", "TrapNet");
        }
        _ => panic!("unknown PDF/UA-1 rule 7.18.2-1 fixture case {case}"),
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 TrapNet annotation fixture");
    bytes
}

pub fn pdfua1_rule_7_20_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 reference XObject fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("PDF/UA-1 fixture page");
    let form_id = document.add_object(Stream::new(
        {
            let mut dictionary = dictionary! {
                "Type" => "XObject",
                "Subtype" => "Form",
                "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
            };
            match case {
                "allowed" => {}
                "forbidden" => dictionary.set("Ref", dictionary! {}),
                _ => panic!("unknown PDF/UA-1 rule 7.20-1 fixture case {case}"),
            }
            dictionary
        },
        Vec::new(),
    ));
    let extra_content_id = document.add_object(Stream::new(
        Dictionary::new(),
        b"/Artifact BMC\n/Fm Do\nEMC\n".to_vec(),
    ));
    let (resources, contents) = {
        let page = document
            .get_object(page_id)
            .expect("PDF/UA-1 fixture page")
            .as_dict()
            .expect("PDF/UA-1 fixture page dictionary");
        (
            page.get(b"Resources").ok().cloned(),
            page.get(b"Contents").ok().cloned(),
        )
    };
    let mut resources = match resources {
        Some(Object::Reference(id)) => document
            .get_object(id)
            .expect("PDF/UA-1 fixture resources")
            .as_dict()
            .expect("PDF/UA-1 fixture resources dictionary")
            .clone(),
        Some(Object::Dictionary(dictionary)) => dictionary,
        _ => Dictionary::new(),
    };
    let mut xobjects = match resources.get(b"XObject").ok().cloned() {
        Some(Object::Reference(id)) => document
            .get_object(id)
            .expect("PDF/UA-1 fixture XObject resources")
            .as_dict()
            .expect("PDF/UA-1 fixture XObject resources dictionary")
            .clone(),
        Some(Object::Dictionary(dictionary)) => dictionary,
        _ => Dictionary::new(),
    };
    xobjects.set("Fm", form_id);
    resources.set("XObject", xobjects);
    let contents = match contents {
        Some(Object::Array(mut contents)) => {
            contents.push(Object::Reference(extra_content_id));
            Object::Array(contents)
        }
        Some(contents) => vec![contents, Object::Reference(extra_content_id)].into(),
        None => Object::Reference(extra_content_id),
    };
    let page = document
        .get_object_mut(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture page dictionary");
    page.set("Resources", resources);
    page.set("Contents", contents);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.20-1 fixture");
    bytes
}

pub fn pdfua1_rule_7_20_2_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 Form XObject structure fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("PDF/UA-1 fixture page");
    let form_id = document.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
            "StructParents" => 1,
        },
        b"/Span <</MCID 0>> BDC\nq\nQ\nEMC\n".to_vec(),
    ));
    let extra_content_id = document.add_object(Stream::new(
        Dictionary::new(),
        match case {
            "allowed" => b"/Span <</MCID 0>> BDC\n/Fm Do\nEMC\n".to_vec(),
            "invalid" => b"/Span <</MCID 0>> BDC\n/Fm Do\n/Fm Do\nEMC\n".to_vec(),
            _ => panic!("unknown PDF/UA-1 rule 7.20-2 fixture case {case}"),
        },
    ));
    let (resources, contents) = {
        let page = document
            .get_object(page_id)
            .expect("PDF/UA-1 fixture page")
            .as_dict()
            .expect("PDF/UA-1 fixture page dictionary");
        (
            page.get(b"Resources").ok().cloned(),
            page.get(b"Contents").ok().cloned(),
        )
    };
    let mut resources = match resources {
        Some(Object::Reference(id)) => document
            .get_object(id)
            .expect("PDF/UA-1 fixture resources")
            .as_dict()
            .expect("PDF/UA-1 fixture resources dictionary")
            .clone(),
        Some(Object::Dictionary(dictionary)) => dictionary,
        _ => Dictionary::new(),
    };
    let mut xobjects = match resources.get(b"XObject").ok().cloned() {
        Some(Object::Reference(id)) => document
            .get_object(id)
            .expect("PDF/UA-1 fixture XObject resources")
            .as_dict()
            .expect("PDF/UA-1 fixture XObject resources dictionary")
            .clone(),
        Some(Object::Dictionary(dictionary)) => dictionary,
        _ => Dictionary::new(),
    };
    xobjects.set("Fm", form_id);
    resources.set("XObject", xobjects);
    let contents = match contents {
        Some(Object::Array(mut contents)) => {
            contents.push(Object::Reference(extra_content_id));
            Object::Array(contents)
        }
        Some(contents) => vec![contents, Object::Reference(extra_content_id)].into(),
        None => Object::Reference(extra_content_id),
    };
    let page = document
        .get_object_mut(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture page dictionary");
    page.set("Resources", resources);
    page.set("Contents", contents);

    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let structure_element_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root object")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .first()
        .expect("PDF/UA-1 fixture structure tree root first kid")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure element");
    let parent_tree = document
        .get_object_mut(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root object")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get_mut(b"ParentTree")
        .expect("PDF/UA-1 fixture parent tree")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture parent tree dictionary");
    let nums = parent_tree
        .get_mut(b"Nums")
        .expect("PDF/UA-1 fixture parent tree numbers")
        .as_array_mut()
        .expect("PDF/UA-1 fixture parent tree numbers array");
    nums.push(1.into());
    nums.push(vec![Object::Reference(structure_element_id)].into());

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.20-2 fixture");
    bytes
}

pub fn pdfua1_rule_7_21_3_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 Type0 font fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("PDF/UA-1 fixture page");
    let (resources, contents) = {
        let page = document
            .get_object(page_id)
            .expect("PDF/UA-1 fixture page")
            .as_dict()
            .expect("PDF/UA-1 fixture page dictionary");
        (
            page.get(b"Resources").ok().cloned(),
            page.get(b"Contents").ok().cloned(),
        )
    };
    let mut resources = match resources {
        Some(Object::Reference(id)) => document
            .get_object(id)
            .expect("PDF/UA-1 fixture resources")
            .as_dict()
            .expect("PDF/UA-1 fixture resources dictionary")
            .clone(),
        Some(Object::Dictionary(dictionary)) => dictionary,
        _ => Dictionary::new(),
    };
    let mut fonts = match resources.get(b"Font").ok().cloned() {
        Some(Object::Reference(id)) => document
            .get_object(id)
            .expect("PDF/UA-1 fixture font resources")
            .as_dict()
            .expect("PDF/UA-1 fixture font resources dictionary")
            .clone(),
        Some(Object::Dictionary(dictionary)) => dictionary,
        _ => Dictionary::new(),
    };
    let descendant_id = type0_descendant_dictionary(&mut document, true, true);
    let (cmap_ordering, cmap_supplement) = match case {
        "identity" => ("Different", 0),
        "matching" => ("Identity", 1),
        "registry_mismatch" => ("Identity", 1),
        "supplement_mismatch" => ("Identity", 0),
        _ => panic!("unknown PDF/UA-1 rule 7.21.3.1-1 fixture case {case}"),
    };
    if case == "registry_mismatch" {
        document
            .get_object_mut(descendant_id)
            .expect("Type0 descendant")
            .as_dict_mut()
            .expect("Type0 descendant dictionary")
            .set(
                "CIDSystemInfo",
                dictionary! {
                    "Registry" => Object::string_literal("Other"),
                    "Ordering" => Object::string_literal("Identity"),
                    "Supplement" => 0,
                },
            );
    } else if case == "supplement_mismatch" {
        document
            .get_object_mut(descendant_id)
            .expect("Type0 descendant")
            .as_dict_mut()
            .expect("Type0 descendant dictionary")
            .set(
                "CIDSystemInfo",
                dictionary! {
                    "Registry" => Object::string_literal("Adobe"),
                    "Ordering" => Object::string_literal("Identity"),
                    "Supplement" => 2,
                },
            );
    }
    let cmap_content =
        b"/CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 1 >> def\n\
/CMapName /Test-CMap def\n\
1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n\
1 begincidrange\n<0000> <FFFF> 0\nendcidrange\n";
    let cmap_id = document.add_object(Stream::new(
        dictionary! {
            "Type" => "CMap",
            "CMapName" => "Test-CMap",
            "CIDSystemInfo" => dictionary! {
                "Registry" => Object::string_literal("Adobe"),
                "Ordering" => Object::string_literal(cmap_ordering),
                "Supplement" => cmap_supplement,
            },
        },
        cmap_content.to_vec(),
    ));
    let encoding = if case == "identity" {
        Object::Name(b"Identity-H".to_vec())
    } else {
        cmap_id.into()
    };
    let font_id = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "MaiTestFont",
        "Encoding" => encoding,
        "DescendantFonts" => vec![Object::Reference(descendant_id)],
    });
    let to_unicode = document.add_object(Stream::new(
        Dictionary::new(),
        b"1 begincodespacerange <0001> <0001> endcodespacerange 1 beginbfchar <0001> <0020> endbfchar".to_vec(),
    ));
    document
        .get_object_mut(font_id)
        .expect("PDF/UA-1 Type0 font")
        .as_dict_mut()
        .expect("PDF/UA-1 Type0 font dictionary")
        .set("ToUnicode", to_unicode);
    fonts.set("FUA", font_id);
    resources.set("Font", fonts);
    let extra_content_id = document.add_object(Stream::new(
        Dictionary::new(),
        b"/Artifact BMC\nBT\n/FUA 12 Tf\n<0001> Tj\nET\nEMC\n".to_vec(),
    ));
    let contents = match contents {
        Some(Object::Array(mut contents)) => {
            contents.push(Object::Reference(extra_content_id));
            Object::Array(contents)
        }
        Some(contents) => vec![contents, Object::Reference(extra_content_id)].into(),
        None => Object::Reference(extra_content_id),
    };
    let page = document
        .get_object_mut(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture page dictionary");
    page.set("Resources", resources);
    page.set("Contents", contents);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 Type0 font fixture");
    bytes
}

pub fn pdfua1_rule_7_21_7_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_21_3_1_fixture("matching"))
        .expect("load PDF/UA-1 Unicode mapping fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("PDF/UA-1 fixture page");
    let font_id = {
        let page = document
            .get_object(page_id)
            .expect("PDF/UA-1 fixture page")
            .as_dict()
            .expect("PDF/UA-1 fixture page dictionary");
        let resources = page
            .get(b"Resources")
            .expect("PDF/UA-1 fixture resources")
            .as_dict()
            .expect("PDF/UA-1 fixture resources dictionary");
        resources
            .get(b"Font")
            .expect("PDF/UA-1 fixture fonts")
            .as_dict()
            .expect("PDF/UA-1 fixture fonts dictionary")
            .get(b"FUA")
            .expect("PDF/UA-1 fixture Type0 font")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture Type0 font")
    };
    match case {
        "matching" => {
            let to_unicode = document.add_object(Stream::new(
                Dictionary::new(),
                b"1 begincodespacerange <0001> <0001> endcodespacerange 1 beginbfchar <0001> <0020> endbfchar".to_vec(),
            ));
            let font = document
                .get_object_mut(font_id)
                .expect("PDF/UA-1 Type0 font")
                .as_dict_mut()
                .expect("PDF/UA-1 Type0 font dictionary");
            font.set("ToUnicode", to_unicode);
        }
        "missing" => {
            document
                .get_object_mut(font_id)
                .expect("PDF/UA-1 Type0 font")
                .as_dict_mut()
                .expect("PDF/UA-1 Type0 font dictionary")
                .remove(b"ToUnicode");
        }
        _ => panic!("unknown PDF/UA-1 rule 7.21.7-1 fixture case {case}"),
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 Unicode mapping fixture");
    bytes
}

pub fn pdfua1_rule_7_21_8_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_21_3_1_fixture("matching"))
        .expect("load PDF/UA-1 .notdef fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("PDF/UA-1 fixture page");
    let (resources, contents) = {
        let page = document
            .get_object(page_id)
            .expect("PDF/UA-1 fixture page")
            .as_dict()
            .expect("PDF/UA-1 fixture page dictionary");
        (
            page.get(b"Resources").ok().cloned(),
            page.get(b"Contents").ok().cloned(),
        )
    };
    let mut resources = match resources {
        Some(Object::Reference(id)) => document
            .get_object(id)
            .expect("PDF/UA-1 fixture resources")
            .as_dict()
            .expect("PDF/UA-1 fixture resources dictionary")
            .clone(),
        Some(Object::Dictionary(dictionary)) => dictionary,
        _ => Dictionary::new(),
    };
    let mut fonts = match resources.get(b"Font").ok().cloned() {
        Some(Object::Reference(id)) => document
            .get_object(id)
            .expect("PDF/UA-1 fixture font resources")
            .as_dict()
            .expect("PDF/UA-1 fixture font resources dictionary")
            .clone(),
        Some(Object::Dictionary(dictionary)) => dictionary,
        _ => Dictionary::new(),
    };
    let mut char_procs = Dictionary::new();
    char_procs.set(
        if case == "fail" { ".notdef" } else { "space" },
        document.add_object(Stream::new(Dictionary::new(), b"500 0 d0\n".to_vec())),
    );
    let encoding_name = if case == "fail" { ".notdef" } else { "space" };
    let to_unicode_id = document.add_object(Stream::new(
        Dictionary::new(),
        b"1 begincodespacerange <20> <20> endcodespacerange 1 beginbfchar <20> <0020> endbfchar"
            .to_vec(),
    ));
    let font_id = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type3",
        "FontBBox" => vec![0.into(), 0.into(), 500.into(), 700.into()],
        "FontMatrix" => vec![0.001.into(), 0.into(), 0.into(), 0.001.into(), 0.into(), 0.into()],
        "CharProcs" => char_procs,
        "Encoding" => dictionary! {
            "Type" => "Encoding",
            "Differences" => vec![32.into(), Object::Name(encoding_name.as_bytes().to_vec())],
        },
        "FirstChar" => 32,
        "LastChar" => 32,
        "Widths" => vec![500.into()],
        "ToUnicode" => to_unicode_id,
    });
    fonts.set("FND", font_id);
    resources.set("Font", fonts);
    let extra_content_id = document.add_object(Stream::new(
        Dictionary::new(),
        b"/Artifact BMC\nBT\n/FND 12 Tf\n3 Tr\n<20> Tj\nET\nEMC\n".to_vec(),
    ));
    let contents = match contents {
        Some(Object::Array(mut contents)) => {
            contents.push(Object::Reference(extra_content_id));
            Object::Array(contents)
        }
        Some(contents) => vec![contents, Object::Reference(extra_content_id)].into(),
        None => Object::Reference(extra_content_id),
    };
    let page = document
        .get_object_mut(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture page dictionary");
    page.set("Resources", resources);
    page.set("Contents", contents);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 .notdef fixture");
    bytes
}

pub fn pdfua1_rule_7_21_7_2_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_21_7_1_fixture("matching"))
        .expect("load PDF/UA-1 Unicode value fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("PDF/UA-1 fixture page");
    let font_id = document
        .get_object(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict()
        .expect("PDF/UA-1 fixture page dictionary")
        .get(b"Resources")
        .expect("PDF/UA-1 fixture resources")
        .as_dict()
        .expect("PDF/UA-1 fixture resources dictionary")
        .get(b"Font")
        .expect("PDF/UA-1 fixture fonts")
        .as_dict()
        .expect("PDF/UA-1 fixture fonts dictionary")
        .get(b"FUA")
        .expect("PDF/UA-1 fixture Type0 font")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture Type0 font");
    let unicode_value = match case {
        "matching" => "0020",
        "zero" => "0000",
        "feff" => "FEFF",
        "fffe" => "FFFE",
        _ => panic!("unknown PDF/UA-1 rule 7.21.7-2 fixture case {case}"),
    };
    let to_unicode = document.add_object(Stream::new(
        Dictionary::new(),
        format!(
            "1 begincodespacerange <0001> <0001> endcodespacerange 1 beginbfchar <0001> <{unicode_value}> endbfchar"
        )
        .into_bytes(),
    ));
    document
        .get_object_mut(font_id)
        .expect("PDF/UA-1 Type0 font")
        .as_dict_mut()
        .expect("PDF/UA-1 Type0 font dictionary")
        .set("ToUnicode", to_unicode);
    save_document(document, "save PDF/UA-1 Unicode value fixture")
}

pub fn pdfua1_rule_7_21_3_2_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_21_3_1_fixture("identity"))
        .expect("load PDF/UA-1 CIDToGIDMap fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("PDF/UA-1 fixture page");
    let font_id = {
        let page = document
            .get_object(page_id)
            .expect("PDF/UA-1 fixture page")
            .as_dict()
            .expect("PDF/UA-1 fixture page dictionary");
        let resources = page
            .get(b"Resources")
            .expect("PDF/UA-1 fixture resources")
            .as_dict()
            .expect("PDF/UA-1 fixture resources dictionary");
        resources
            .get(b"Font")
            .expect("PDF/UA-1 fixture fonts")
            .as_dict()
            .expect("PDF/UA-1 fixture fonts dictionary")
            .get(b"FUA")
            .expect("PDF/UA-1 fixture Type0 font")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture Type0 font")
    };
    let descendant_id = document
        .get_object(font_id)
        .expect("PDF/UA-1 Type0 font")
        .as_dict()
        .expect("PDF/UA-1 Type0 font dictionary")
        .get(b"DescendantFonts")
        .expect("PDF/UA-1 Type0 descendants")
        .as_array()
        .expect("PDF/UA-1 Type0 descendants array")
        .first()
        .expect("PDF/UA-1 Type0 first descendant")
        .as_reference()
        .expect("indirect PDF/UA-1 Type0 descendant");
    let descendant = document
        .get_object_mut(descendant_id)
        .expect("PDF/UA-1 Type0 descendant")
        .as_dict_mut()
        .expect("PDF/UA-1 Type0 descendant dictionary");
    match case {
        "identity" => {}
        "stream" => descendant.set("CIDToGIDMap", Stream::new(Dictionary::new(), vec![0, 0])),
        "missing" => {
            descendant.remove(b"CIDToGIDMap");
        }
        "invalid" => descendant.set("CIDToGIDMap", 7),
        _ => panic!("unknown PDF/UA-1 rule 7.21.3.2-1 fixture case {case}"),
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 CIDToGIDMap fixture");
    bytes
}

pub fn pdfua1_rule_7_21_3_3_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_21_3_1_fixture("matching"))
        .expect("load PDF/UA-1 CMap embedding fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("PDF/UA-1 fixture page");
    let font_id = {
        let page = document
            .get_object(page_id)
            .expect("PDF/UA-1 fixture page")
            .as_dict()
            .expect("PDF/UA-1 fixture page dictionary");
        page.get(b"Resources")
            .expect("PDF/UA-1 fixture resources")
            .as_dict()
            .expect("PDF/UA-1 fixture resources dictionary")
            .get(b"Font")
            .expect("PDF/UA-1 fixture fonts")
            .as_dict()
            .expect("PDF/UA-1 fixture fonts dictionary")
            .get(b"FUA")
            .expect("PDF/UA-1 fixture Type0 font")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture Type0 font")
    };
    let encoding = match case {
        "embedded" => return save_document(document, "PDF/UA-1 CMap embedding fixture"),
        "predefined" => Object::Name(b"GB-EUC-H".to_vec()),
        "unembedded" => Object::Name(b"Test-CMap".to_vec()),
        _ => panic!("unknown PDF/UA-1 rule 7.21.3.3-1 fixture case {case}"),
    };
    if case == "predefined" {
        let descendant_id = document
            .get_object(font_id)
            .expect("PDF/UA-1 Type0 font")
            .as_dict()
            .expect("PDF/UA-1 Type0 font dictionary")
            .get(b"DescendantFonts")
            .expect("PDF/UA-1 Type0 descendants")
            .as_array()
            .expect("PDF/UA-1 Type0 descendants array")
            .first()
            .expect("PDF/UA-1 Type0 first descendant")
            .as_reference()
            .expect("indirect PDF/UA-1 Type0 descendant");
        document
            .get_object_mut(descendant_id)
            .expect("PDF/UA-1 Type0 descendant")
            .as_dict_mut()
            .expect("PDF/UA-1 Type0 descendant dictionary")
            .set(
                "CIDSystemInfo",
                dictionary! {
                    "Registry" => Object::string_literal("Adobe"),
                    "Ordering" => Object::string_literal("GB1"),
                    "Supplement" => 0,
                },
            );
    }
    document
        .get_object_mut(font_id)
        .expect("PDF/UA-1 Type0 font")
        .as_dict_mut()
        .expect("PDF/UA-1 Type0 font dictionary")
        .set("Encoding", encoding);
    save_document(document, "PDF/UA-1 CMap embedding fixture")
}

pub fn pdfua1_rule_7_21_3_3_2_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_21_3_1_fixture("matching"))
        .expect("load PDF/UA-1 CMap WMode fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("PDF/UA-1 fixture page");
    let font_id = document
        .get_object(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict()
        .expect("PDF/UA-1 fixture page dictionary")
        .get(b"Resources")
        .expect("PDF/UA-1 fixture resources")
        .as_dict()
        .expect("PDF/UA-1 fixture resources dictionary")
        .get(b"Font")
        .expect("PDF/UA-1 fixture fonts")
        .as_dict()
        .expect("PDF/UA-1 fixture fonts dictionary")
        .get(b"FUA")
        .expect("PDF/UA-1 fixture Type0 font")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture Type0 font");
    let cmap_id = document
        .get_object(font_id)
        .expect("PDF/UA-1 Type0 font")
        .as_dict()
        .expect("PDF/UA-1 Type0 font dictionary")
        .get(b"Encoding")
        .expect("PDF/UA-1 Type0 encoding")
        .as_reference()
        .expect("indirect PDF/UA-1 CMap");
    let content_wmode = match case {
        "matching" | "mismatched" => 0,
        _ => panic!("unknown PDF/UA-1 rule 7.21.3.3-2 fixture case {case}"),
    };
    let dictionary_wmode = if case == "mismatched" { 1 } else { 0 };
    let cmap = document
        .get_object_mut(cmap_id)
        .expect("PDF/UA-1 CMap")
        .as_stream_mut()
        .expect("PDF/UA-1 CMap stream");
    cmap.dict.set("WMode", dictionary_wmode);
    cmap.set_content(embedded_cmap("Identity", content_wmode, 0));
    let to_unicode = document.add_object(Stream::new(
        Dictionary::new(),
        b"1 begincodespacerange <00> <ff> endcodespacerange 2 beginbfchar <00> <0020> <01> <0020> endbfchar"
            .to_vec(),
    ));
    document
        .get_object_mut(font_id)
        .expect("PDF/UA-1 Type0 font")
        .as_dict_mut()
        .expect("PDF/UA-1 Type0 font dictionary")
        .set("ToUnicode", to_unicode);
    let content_id = document
        .get_object(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict()
        .expect("PDF/UA-1 fixture page dictionary")
        .get(b"Contents")
        .expect("PDF/UA-1 fixture contents")
        .as_array()
        .expect("PDF/UA-1 fixture contents array")
        .last()
        .expect("PDF/UA-1 fixture final content stream")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture final content stream");
    document
        .get_object_mut(content_id)
        .expect("PDF/UA-1 fixture final content stream")
        .as_stream_mut()
        .expect("PDF/UA-1 fixture final content stream dictionary")
        .set_content(b"/Artifact BMC\nBT\n/FUA 12 Tf\n<01> Tj\nET\nEMC\n".to_vec());
    save_document(document, "PDF/UA-1 CMap WMode fixture")
}

pub fn pdfua1_rule_7_21_3_3_3_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_21_3_3_2_fixture("matching"))
        .expect("load PDF/UA-1 CMap reference fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("PDF/UA-1 fixture page");
    let font_id = document
        .get_object(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict()
        .expect("PDF/UA-1 fixture page dictionary")
        .get(b"Resources")
        .expect("PDF/UA-1 fixture resources")
        .as_dict()
        .expect("PDF/UA-1 fixture resources dictionary")
        .get(b"Font")
        .expect("PDF/UA-1 fixture fonts")
        .as_dict()
        .expect("PDF/UA-1 fixture fonts dictionary")
        .get(b"FUA")
        .expect("PDF/UA-1 fixture Type0 font")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture Type0 font");
    let cmap_id = document
        .get_object(font_id)
        .expect("PDF/UA-1 Type0 font")
        .as_dict()
        .expect("PDF/UA-1 Type0 font dictionary")
        .get(b"Encoding")
        .expect("PDF/UA-1 Type0 encoding")
        .as_reference()
        .expect("indirect PDF/UA-1 CMap");
    let dictionary_unknown_reference = (case == "dictionary_unknown").then(|| {
        document.add_object(Stream::new(
            dictionary! {
                "Type" => "CMap",
                "CMapName" => "NotAStandardCMap",
            },
            embedded_cmap("Identity", 0, 0),
        ))
    });
    let cmap = document
        .get_object_mut(cmap_id)
        .expect("PDF/UA-1 CMap")
        .as_stream_mut()
        .expect("PDF/UA-1 CMap stream");
    match case {
        "allowed" => cmap.set_content(embedded_identity_usecmap("Identity", 0)),
        "embedded_unknown" => cmap.set_content(embedded_unknown_usecmap("Identity", 0)),
        "dictionary_unknown" => {
            cmap.dict.set(
                "UseCMap",
                dictionary_unknown_reference.expect("unknown CMap"),
            );
        }
        _ => panic!("unknown PDF/UA-1 rule 7.21.3.3-3 fixture case {case}"),
    }
    let to_unicode_content = if case == "allowed" {
        b"1 begincodespacerange <0000> <ffff> endcodespacerange 1 beginbfchar <0001> <0020> endbfchar".as_slice()
    } else {
        b"1 begincodespacerange <00> <ff> endcodespacerange 2 beginbfchar <00> <0020> <01> <0020> endbfchar".as_slice()
    };
    let to_unicode =
        document.add_object(Stream::new(Dictionary::new(), to_unicode_content.to_vec()));
    document
        .get_object_mut(font_id)
        .expect("PDF/UA-1 Type0 font")
        .as_dict_mut()
        .expect("PDF/UA-1 Type0 font dictionary")
        .set("ToUnicode", to_unicode);
    save_document(document, "PDF/UA-1 CMap reference fixture")
}

pub fn pdfua1_rule_7_21_4_1_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_21_3_1_fixture("matching"))
        .expect("load PDF/UA-1 font embedding fixture");
    if case == "unembedded" {
        let page_id = *document
            .get_pages()
            .values()
            .next()
            .expect("PDF/UA-1 fixture page");
        let font_id = document
            .get_object(page_id)
            .expect("PDF/UA-1 fixture page")
            .as_dict()
            .expect("PDF/UA-1 fixture page dictionary")
            .get(b"Resources")
            .expect("PDF/UA-1 fixture resources")
            .as_dict()
            .expect("PDF/UA-1 fixture resources dictionary")
            .get(b"Font")
            .expect("PDF/UA-1 fixture fonts")
            .as_dict()
            .expect("PDF/UA-1 fixture fonts dictionary")
            .get(b"FUA")
            .expect("PDF/UA-1 fixture Type0 font")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture Type0 font");
        let descendant_id = document
            .get_object(font_id)
            .expect("PDF/UA-1 Type0 font")
            .as_dict()
            .expect("PDF/UA-1 Type0 font dictionary")
            .get(b"DescendantFonts")
            .expect("PDF/UA-1 Type0 descendants")
            .as_array()
            .expect("PDF/UA-1 Type0 descendants array")
            .first()
            .expect("PDF/UA-1 Type0 first descendant")
            .as_reference()
            .expect("indirect PDF/UA-1 Type0 descendant");
        let descriptor_id = document
            .get_object(descendant_id)
            .expect("PDF/UA-1 Type0 descendant")
            .as_dict()
            .expect("PDF/UA-1 Type0 descendant dictionary")
            .get(b"FontDescriptor")
            .expect("PDF/UA-1 Type0 font descriptor")
            .as_reference()
            .expect("indirect PDF/UA-1 Type0 font descriptor");
        document
            .get_object_mut(descriptor_id)
            .expect("PDF/UA-1 Type0 font descriptor")
            .as_dict_mut()
            .expect("PDF/UA-1 Type0 font descriptor dictionary")
            .remove(b"FontFile2");
    } else if case != "embedded" {
        panic!("unknown PDF/UA-1 rule 7.21.4.1-1 fixture case {case}");
    }
    save_document(document, "PDF/UA-1 font embedding fixture")
}

pub fn pdfua1_rule_7_21_4_1_2_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_21_3_1_fixture("matching"))
        .expect("load PDF/UA-1 glyph presence fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("PDF/UA-1 fixture page");
    let mut resources = document
        .get_object(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict()
        .expect("PDF/UA-1 fixture page dictionary")
        .get(b"Resources")
        .expect("PDF/UA-1 fixture resources")
        .as_dict()
        .expect("PDF/UA-1 fixture resources dictionary")
        .clone();
    let mut fonts = resources
        .get(b"Font")
        .expect("PDF/UA-1 fixture fonts")
        .as_dict()
        .expect("PDF/UA-1 fixture fonts dictionary")
        .clone();
    let glyph_count = match case {
        "present" | "invisible" => 2,
        "missing" => 1,
        _ => panic!("unknown PDF/UA-1 rule 7.21.4.1-2 fixture case {case}"),
    };
    let mut descriptor = font_descriptor(&mut document, false);
    descriptor.set(
        "FontFile2",
        document.add_object(Stream::new(
            Dictionary::new(),
            sfnt::minimal_truetype_with_cmap_count_and_mapping_and_glyph_count(1, 33, glyph_count),
        )),
    );
    let descriptor_id = document.add_object(descriptor);
    let to_unicode = document.add_object(Stream::new(
        Dictionary::new(),
        b"1 begincodespacerange <00> <ff> endcodespacerange 1 beginbfchar <21> <0021> endbfchar"
            .to_vec(),
    ));
    let font_id = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "TrueType",
        "BaseFont" => "MaiTestFont",
        "Encoding" => "WinAnsiEncoding",
        "FirstChar" => 33,
        "LastChar" => 33,
        "Widths" => vec![500.into()],
        "FontDescriptor" => descriptor_id,
        "ToUnicode" => to_unicode,
    });
    fonts.set("FTT", font_id);
    resources.set("Font", fonts);
    let content = if case == "invisible" {
        b"BT /FTT 12 Tf 3 Tr (!) Tj ET".to_vec()
    } else {
        b"BT /FTT 12 Tf (!) Tj ET".to_vec()
    };
    let content_id = document.add_object(Stream::new(Dictionary::new(), content));
    let page = document
        .get_object_mut(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture page dictionary");
    page.set("Resources", resources);
    page.set("Contents", content_id);
    save_document(document, "PDF/UA-1 glyph presence fixture")
}

pub fn pdfua1_rule_7_21_5_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_21_3_1_fixture("matching"))
        .expect("load PDF/UA-1 glyph width fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("PDF/UA-1 fixture page");
    let font_id = document
        .get_object(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict()
        .expect("PDF/UA-1 fixture page dictionary")
        .get(b"Resources")
        .expect("PDF/UA-1 fixture resources")
        .as_dict()
        .expect("PDF/UA-1 fixture resources dictionary")
        .get(b"Font")
        .expect("PDF/UA-1 fixture fonts")
        .as_dict()
        .expect("PDF/UA-1 fixture fonts dictionary")
        .get(b"FUA")
        .expect("PDF/UA-1 fixture Type0 font")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture Type0 font");
    let descendant_id = document
        .get_object(font_id)
        .expect("PDF/UA-1 Type0 font")
        .as_dict()
        .expect("PDF/UA-1 Type0 font dictionary")
        .get(b"DescendantFonts")
        .expect("PDF/UA-1 Type0 descendants")
        .as_array()
        .expect("PDF/UA-1 Type0 descendants array")
        .first()
        .expect("PDF/UA-1 Type0 first descendant")
        .as_reference()
        .expect("indirect PDF/UA-1 Type0 descendant");
    match case {
        "matching" => {}
        "mismatched" => document
            .get_object_mut(descendant_id)
            .expect("PDF/UA-1 Type0 descendant")
            .as_dict_mut()
            .expect("PDF/UA-1 Type0 descendant dictionary")
            .set("DW", 400),
        _ => panic!("unknown PDF/UA-1 rule 7.21.5-1 fixture case {case}"),
    }
    save_document(document, "PDF/UA-1 glyph width fixture")
}

pub fn pdfua1_rule_7_21_6_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_21_3_1_fixture("matching"))
        .expect("load PDF/UA-1 TrueType cmap fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("PDF/UA-1 fixture page");
    let page = document
        .get_object(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict()
        .expect("PDF/UA-1 fixture page dictionary");
    let mut resources = page
        .get(b"Resources")
        .expect("PDF/UA-1 fixture resources")
        .as_dict()
        .expect("PDF/UA-1 fixture resources dictionary")
        .clone();
    let mut fonts = resources
        .get(b"Font")
        .expect("PDF/UA-1 fixture fonts")
        .as_dict()
        .expect("PDF/UA-1 fixture fonts dictionary")
        .clone();
    let cmap_count = match case {
        "matching" => 1,
        "missing" => 0,
        _ => panic!("unknown PDF/UA-1 rule 7.21.6-1 fixture case {case}"),
    };
    let mut descriptor = font_descriptor(&mut document, false);
    descriptor.set(
        "FontFile2",
        document.add_object(Stream::new(
            Dictionary::new(),
            sfnt::minimal_truetype_with_cmap_count(cmap_count),
        )),
    );
    let descriptor_id = document.add_object(descriptor);
    let font_id = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "TrueType",
        "BaseFont" => "MaiTestFont",
        "Encoding" => "WinAnsiEncoding",
        "FirstChar" => 33,
        "LastChar" => 33,
        "Widths" => vec![500.into()],
        "FontDescriptor" => descriptor_id,
    });
    fonts.set("FTT", font_id);
    resources.set("Font", fonts);
    let content_id = document.add_object(Stream::new(
        Dictionary::new(),
        b"/Artifact BMC BT /FTT 12 Tf 3 Tr (!) Tj ET EMC".to_vec(),
    ));
    let page = document
        .get_object_mut(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture page dictionary");
    page.set("Resources", resources);
    page.set("Contents", content_id);
    save_document(document, "PDF/UA-1 TrueType cmap fixture")
}

pub fn pdfua1_rule_7_21_6_3_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_21_6_1_fixture("matching"))
        .expect("load PDF/UA-1 symbolic TrueType encoding fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("PDF/UA-1 fixture page");
    let font_id = document
        .get_object(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict()
        .expect("PDF/UA-1 fixture page dictionary")
        .get(b"Resources")
        .expect("PDF/UA-1 fixture resources")
        .as_dict()
        .expect("PDF/UA-1 fixture resources dictionary")
        .get(b"Font")
        .expect("PDF/UA-1 fixture fonts")
        .as_dict()
        .expect("PDF/UA-1 fixture fonts dictionary")
        .get(b"FTT")
        .expect("PDF/UA-1 fixture TrueType font")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture TrueType font");
    let descriptor_id = document
        .get_object(font_id)
        .expect("PDF/UA-1 fixture TrueType font")
        .as_dict()
        .expect("PDF/UA-1 fixture TrueType font dictionary")
        .get(b"FontDescriptor")
        .expect("PDF/UA-1 TrueType descriptor")
        .as_reference()
        .expect("indirect PDF/UA-1 TrueType descriptor");
    document
        .get_object_mut(descriptor_id)
        .expect("PDF/UA-1 TrueType descriptor")
        .as_dict_mut()
        .expect("PDF/UA-1 TrueType descriptor dictionary")
        .set("Flags", 4);
    let font = document
        .get_object_mut(font_id)
        .expect("PDF/UA-1 fixture TrueType font")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture TrueType font dictionary");
    match case {
        "matching" => {
            font.remove(b"Encoding");
        }
        "encoding" => {}
        _ => panic!("unknown PDF/UA-1 rule 7.21.6-3 fixture case {case}"),
    }
    save_document(document, "PDF/UA-1 symbolic TrueType encoding fixture")
}

pub fn pdfua1_rule_7_21_6_4_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_21_6_3_fixture("matching"))
        .expect("load PDF/UA-1 symbolic TrueType cmap fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("PDF/UA-1 fixture page");
    let font_id = document
        .get_object(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict()
        .expect("PDF/UA-1 fixture page dictionary")
        .get(b"Resources")
        .expect("PDF/UA-1 fixture resources")
        .as_dict()
        .expect("PDF/UA-1 fixture resources dictionary")
        .get(b"Font")
        .expect("PDF/UA-1 fixture fonts")
        .as_dict()
        .expect("PDF/UA-1 fixture fonts dictionary")
        .get(b"FTT")
        .expect("PDF/UA-1 fixture TrueType font")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture TrueType font");
    let descriptor_id = document
        .get_object(font_id)
        .expect("PDF/UA-1 fixture TrueType font")
        .as_dict()
        .expect("PDF/UA-1 fixture TrueType font dictionary")
        .get(b"FontDescriptor")
        .expect("PDF/UA-1 TrueType descriptor")
        .as_reference()
        .expect("indirect PDF/UA-1 TrueType descriptor");
    let font_program = match case {
        "one_cmap" => sfnt::minimal_truetype_with_cmap_count(1),
        "two_cmaps" => sfnt::minimal_truetype_with_cmap_count(2),
        "two_cmaps_with_cmap30" => sfnt::minimal_truetype_with_symbol_cmap(2),
        _ => panic!("unknown PDF/UA-1 rule 7.21.6-4 fixture case {case}"),
    };
    let font_program_id = document.add_object(Stream::new(Dictionary::new(), font_program));
    document
        .get_object_mut(descriptor_id)
        .expect("PDF/UA-1 TrueType descriptor")
        .as_dict_mut()
        .expect("PDF/UA-1 TrueType descriptor dictionary")
        .set("FontFile2", font_program_id);
    save_document(document, "PDF/UA-1 symbolic TrueType cmap fixture")
}

pub fn pdfua1_rule_7_21_6_2_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_21_6_1_fixture("matching"))
        .expect("load PDF/UA-1 TrueType encoding fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("PDF/UA-1 fixture page");
    let (font_id, resources) = {
        let page = document
            .get_object(page_id)
            .expect("PDF/UA-1 fixture page")
            .as_dict()
            .expect("PDF/UA-1 fixture page dictionary");
        (
            page.get(b"Resources")
                .expect("PDF/UA-1 fixture resources")
                .as_dict()
                .expect("PDF/UA-1 fixture resources dictionary")
                .get(b"Font")
                .expect("PDF/UA-1 fixture fonts")
                .as_dict()
                .expect("PDF/UA-1 fixture fonts dictionary")
                .get(b"FTT")
                .expect("PDF/UA-1 fixture TrueType font")
                .as_reference()
                .expect("indirect PDF/UA-1 fixture TrueType font"),
            page.get(b"Resources")
                .expect("PDF/UA-1 fixture resources")
                .as_dict()
                .expect("PDF/UA-1 fixture resources dictionary")
                .clone(),
        )
    };
    let descriptor_id = if case == "missing_unicode_cmap" {
        Some(
            document
                .get_object(font_id)
                .expect("PDF/UA-1 fixture TrueType font")
                .as_dict()
                .expect("PDF/UA-1 fixture TrueType font dictionary")
                .get(b"FontDescriptor")
                .expect("PDF/UA-1 TrueType descriptor")
                .as_reference()
                .expect("indirect PDF/UA-1 TrueType descriptor"),
        )
    } else {
        None
    };
    let replacement_font_file = (case == "missing_unicode_cmap").then(|| {
        document.add_object(Stream::new(
            Dictionary::new(),
            sfnt::minimal_truetype_with_cmap_encoding(2),
        ))
    });
    match case {
        "matching" => {}
        "invalid_encoding" => document
            .get_object_mut(font_id)
            .expect("PDF/UA-1 fixture TrueType font")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture TrueType font dictionary")
            .set("Encoding", "StandardEncoding"),
        "invalid_differences" => document
            .get_object_mut(font_id)
            .expect("PDF/UA-1 fixture TrueType font")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture TrueType font dictionary")
            .set(
                "Encoding",
                dictionary! {
                    "Type" => "Encoding",
                    "BaseEncoding" => "WinAnsiEncoding",
                    "Differences" => vec![32.into(), Object::Name(b"notAnAdobeGlyph".to_vec())],
                },
            ),
        "missing_unicode_cmap" => {
            document
                .get_object_mut(descriptor_id.expect("missing Unicode descriptor"))
                .expect("PDF/UA-1 TrueType descriptor")
                .as_dict_mut()
                .expect("PDF/UA-1 TrueType descriptor dictionary")
                .set(
                    "FontFile2",
                    replacement_font_file.expect("missing Unicode font program"),
                );
        }
        _ => panic!("unknown PDF/UA-1 rule 7.21.6-2 fixture case {case}"),
    }
    let page = document
        .get_object_mut(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture page dictionary");
    page.set("Resources", resources);
    save_document(document, "PDF/UA-1 TrueType encoding fixture")
}

pub fn pdfua1_rule_7_21_4_2_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 Type1 CharSet fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("PDF/UA-1 fixture page");
    let page = document
        .get_object(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict()
        .expect("PDF/UA-1 fixture page dictionary");
    let mut resources = match page.get(b"Resources").expect("PDF/UA-1 fixture resources") {
        Object::Reference(id) => document
            .get_object(*id)
            .expect("PDF/UA-1 fixture resources")
            .as_dict()
            .expect("PDF/UA-1 fixture resources dictionary")
            .clone(),
        Object::Dictionary(resources) => resources.clone(),
        _ => panic!("PDF/UA-1 fixture resources dictionary has unexpected type"),
    };
    let mut fonts = match resources.get(b"Font").expect("PDF/UA-1 fixture fonts") {
        Object::Reference(id) => document
            .get_object(*id)
            .expect("PDF/UA-1 fixture fonts")
            .as_dict()
            .expect("PDF/UA-1 fixture fonts dictionary")
            .clone(),
        Object::Dictionary(fonts) => fonts.clone(),
        _ => panic!("PDF/UA-1 fixture fonts dictionary has unexpected type"),
    };
    let (program, length1, length2, length3) =
        pdf_type1_program(include_bytes!("../fixtures/fonts/usyr.pfa"));
    let mut descriptor = font_descriptor(&mut document, false);
    descriptor.set("FontName", "StandardSymL");
    descriptor.set(
        "FontFile",
        document.add_object(Stream::new(
            dictionary! {
                "Length1" => i64::try_from(length1).expect("Type1 clear length"),
                "Length2" => i64::try_from(length2).expect("Type1 encrypted length"),
                "Length3" => i64::try_from(length3).expect("Type1 trailer length"),
            },
            program,
        )),
    );
    descriptor.set(
        "CharSet",
        Object::string_literal(if case == "complete" {
            "/space/exclam/universal/numbersign/existential/percent/ampersand/suchthat/parenleft/parenright/asteriskmath/plus/comma/minus/period/slash/zero/one/two/three/four/five/six/seven/eight/nine/colon/semicolon/less/equal/greater/question/congruent/Alpha/Beta/Chi/Delta/Epsilon/Phi/Gamma/Eta/Iota/theta1/Kappa/Lambda/Mu/Nu/Omicron/Pi/Theta/Rho/Sigma/Tau/Upsilon/sigma1/Omega/Xi/Psi/Zeta/bracketleft/therefore/bracketright/perpendicular/underscore/radicalex/alpha/beta/chi/delta/epsilon/phi/gamma/eta/iota/phi1/kappa/lambda/mu/nu/omicron/pi/theta/rho/sigma/tau/upsilon/omega1/omega/xi/psi/zeta/braceleft/bar/braceright/similar/Upsilon1/Euro/minute/lessequal/fraction/infinity/florin/club/diamond/heart/spade/arrowboth/arrowleft/arrowup/arrowright/arrowdown/degree/plusminus/second/greaterequal/multiply/proportional/partialdiff/bullet/divide/notequal/equivalence/approxequal/ellipsis/arrowvertex/arrowhorizex/carriagereturn/aleph/Ifraktur/Rfraktur/weierstrass/circlemultiply/circleplus/emptyset/intersection/union/propersuperset/reflexsuperset/notsubset/propersubset/reflexsubset/element/notelement/angle/gradient/registerserif/copyrightserif/trademarkserif/product/radical/dotmath/logicalnot/logicaland/logicalor/arrowdblboth/arrowdblleft/arrowdblup/arrowdblright/arrowdbldown/lozenge/angleleft/registersans/copyrightsans/trademarksans/summation/parenlefttp/parenleftex/parenleftbt/bracketlefttp/bracketleftex/bracketleftbt/bracelefttp/braceleftmid/braceleftbt/braceex/angleright/integral/integraltp/integralex/integralbt/parenrighttp/parenrightex/parenrightbt/bracketrighttp/bracerighttp/bracerightmid/bracerightbt/.notdef"
        } else {
            "/.notdef"
        }),
    );
    let descriptor_id = document.add_object(descriptor);
    let to_unicode = document.add_object(Stream::new(
        Dictionary::new(),
        b"1 begincodespacerange <22> <22> endcodespacerange 1 beginbfchar <22> <0021> endbfchar"
            .to_vec(),
    ));
    let font_id = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "ABCDEF+StandardSymL",
        "Encoding" => dictionary! {
            "Differences" => vec![34.into(), Object::Name(b"universal".to_vec())],
        },
        "FirstChar" => 34,
        "LastChar" => 34,
        "Widths" => vec![713.into()],
        "FontDescriptor" => descriptor_id,
        "ToUnicode" => to_unicode,
    });
    fonts.set("FT1", font_id);
    resources.set("Font", fonts);
    let content_id = document.add_object(Stream::new(
        Dictionary::new(),
        b"/Artifact BMC BT /FT1 12 Tf <22> Tj ET EMC".to_vec(),
    ));
    let page = document
        .get_object_mut(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture page dictionary");
    page.set("Resources", resources);
    let contents = page
        .get(b"Contents")
        .expect("PDF/UA-1 fixture contents")
        .clone();
    page.set("Contents", vec![contents, Object::Reference(content_id)]);
    save_document(document, "PDF/UA-1 Type1 CharSet fixture")
}

pub fn pdfua1_rule_7_21_4_2_2_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_21_3_1_fixture("identity"))
        .expect("load PDF/UA-1 CIDSet fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("PDF/UA-1 fixture page");
    let font_id = document
        .get_object(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict()
        .expect("PDF/UA-1 fixture page dictionary")
        .get(b"Resources")
        .expect("PDF/UA-1 fixture resources")
        .as_dict()
        .expect("PDF/UA-1 fixture resources dictionary")
        .get(b"Font")
        .expect("PDF/UA-1 fixture fonts")
        .as_dict()
        .expect("PDF/UA-1 fixture fonts dictionary")
        .get(b"FUA")
        .expect("PDF/UA-1 fixture Type0 font")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture Type0 font");
    let descendant_id = document
        .get_object(font_id)
        .expect("PDF/UA-1 Type0 font")
        .as_dict()
        .expect("PDF/UA-1 Type0 font dictionary")
        .get(b"DescendantFonts")
        .expect("PDF/UA-1 Type0 descendants")
        .as_array()
        .expect("PDF/UA-1 Type0 descendants array")
        .first()
        .expect("PDF/UA-1 Type0 first descendant")
        .as_reference()
        .expect("indirect PDF/UA-1 Type0 descendant");
    let descriptor_id = document
        .get_object(descendant_id)
        .expect("PDF/UA-1 Type0 descendant")
        .as_dict()
        .expect("PDF/UA-1 Type0 descendant dictionary")
        .get(b"FontDescriptor")
        .expect("PDF/UA-1 Type0 font descriptor")
        .as_reference()
        .expect("indirect PDF/UA-1 Type0 font descriptor");
    let descendant = document
        .get_object_mut(descendant_id)
        .expect("PDF/UA-1 Type0 descendant")
        .as_dict_mut()
        .expect("PDF/UA-1 Type0 descendant dictionary");
    descendant.set("BaseFont", "ABCDEF+MaiTestFont");
    let cid_set = match case {
        "complete" => vec![0xc0],
        "incomplete" => vec![0x80],
        _ => panic!("unknown PDF/UA-1 rule 7.21.4.2-2 fixture case {case}"),
    };
    let cid_set_id = document.add_object(Stream::new(Dictionary::new(), cid_set));
    document
        .get_object_mut(descriptor_id)
        .expect("PDF/UA-1 Type0 font descriptor")
        .as_dict_mut()
        .expect("PDF/UA-1 Type0 font descriptor dictionary")
        .set("CIDSet", cid_set_id);

    // Render only CID 1 so CID 0 remains the font program's unreferenced
    // .notdef glyph and the missing CID 1 in the failing fixture is still
    // intentionally covered by the CIDSet check.
    let content_ids = document
        .objects
        .iter()
        .filter_map(|(object_id, object)| {
            let stream = object.as_stream().ok()?;
            let bytes = stream.decompressed_content().ok()?;
            bytes
                .windows(b"/FUA".len())
                .any(|window| window == b"/FUA")
                .then_some(*object_id)
        })
        .collect::<Vec<_>>();
    for content_id in content_ids {
        document
            .get_object_mut(content_id)
            .expect("PDF/UA-1 Type0 content stream")
            .as_stream_mut()
            .expect("PDF/UA-1 Type0 content stream")
            .set_content(b"/Artifact BMC\nBT\n/FUA 12 Tf\n<0001> Tj\nET\nEMC\n".to_vec());
    }
    let to_unicode = document.add_object(Stream::new(
        Dictionary::new(),
        b"1 begincodespacerange <0001> <0001> endcodespacerange 1 beginbfchar <0001> <0020> endbfchar"
            .to_vec(),
    ));
    document
        .get_object_mut(font_id)
        .expect("PDF/UA-1 Type0 font")
        .as_dict_mut()
        .expect("PDF/UA-1 Type0 font dictionary")
        .set("ToUnicode", to_unicode);
    save_document(document, "PDF/UA-1 CIDSet fixture")
}

fn save_document(mut document: Document, description: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).expect(description);
    bytes
}

pub fn pdfua1_rule_7_18_3_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_18_1_1_fixture("valid"))
        .expect("load PDF/UA-1 page Tabs fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let pages_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"Pages")
        .expect("PDF/UA-1 fixture pages")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture pages");
    let page_id = document
        .get_object(pages_id)
        .expect("PDF/UA-1 fixture pages object")
        .as_dict()
        .expect("PDF/UA-1 fixture pages dictionary")
        .get(b"Kids")
        .expect("PDF/UA-1 fixture page kids")
        .as_array()
        .expect("PDF/UA-1 fixture page kids array")
        .first()
        .expect("PDF/UA-1 fixture page")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture page");
    let page = document
        .get_object_mut(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture page dictionary");
    match case {
        "allowed" => page.set("Tabs", "S"),
        "missing" => {
            page.remove(b"Tabs");
        }
        "wrong" => page.set("Tabs", "R"),
        _ => panic!("unknown PDF/UA-1 rule 7.18.3-1 fixture case {case}"),
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 page Tabs fixture");
    bytes
}

pub fn pdfua1_rule_7_18_4_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_18_1_3_fixture("tu"))
        .expect("load PDF/UA-1 Widget Form-tag fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let structure_form_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"ParentTree")
        .expect("PDF/UA-1 fixture parent tree")
        .as_dict()
        .expect("PDF/UA-1 fixture parent tree dictionary")
        .get(b"Nums")
        .expect("PDF/UA-1 fixture parent tree numbers")
        .as_array()
        .expect("PDF/UA-1 fixture parent tree numbers array")
        .chunks(2)
        .find_map(|pair| {
            (pair.first()?.as_i64().ok()? == 200)
                .then(|| pair.get(1)?.as_reference().ok())
                .flatten()
        })
        .expect("PDF/UA-1 fixture Widget structure element");
    match case {
        "allowed" => {}
        "role_mapped" => {
            document
                .get_object_mut(struct_tree_root_id)
                .expect("PDF/UA-1 fixture structure tree root")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture structure tree root dictionary")
                .set("RoleMap", dictionary! {"CustomForm" => "Form"});
            document
                .get_object_mut(structure_form_id)
                .expect("PDF/UA-1 fixture Widget structure element")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture Widget structure element dictionary")
                .set("S", "CustomForm");
        }
        "not_nested" => {
            document
                .get_object_mut(structure_form_id)
                .expect("PDF/UA-1 fixture Widget structure element")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture Widget structure element dictionary")
                .set("S", "P");
        }
        _ => panic!("unknown PDF/UA-1 rule 7.18.4-1 fixture case {case}"),
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 Widget Form-tag fixture");
    bytes
}

pub fn pdfua1_rule_7_18_4_2_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_18_1_3_fixture("tu"))
        .expect("load PDF/UA-1 Form child fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let structure_form_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"ParentTree")
        .expect("PDF/UA-1 fixture parent tree")
        .as_dict()
        .expect("PDF/UA-1 fixture parent tree dictionary")
        .get(b"Nums")
        .expect("PDF/UA-1 fixture parent tree numbers")
        .as_array()
        .expect("PDF/UA-1 fixture parent tree numbers array")
        .chunks(2)
        .find_map(|pair| {
            (pair.first()?.as_i64().ok()? == 200)
                .then(|| pair.get(1)?.as_reference().ok())
                .flatten()
        })
        .expect("PDF/UA-1 fixture Form structure element");
    let child = document
        .get_object(structure_form_id)
        .expect("PDF/UA-1 fixture Form structure element")
        .as_dict()
        .expect("PDF/UA-1 fixture Form structure element dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture Form child")
        .clone();
    let structure_form = document
        .get_object_mut(structure_form_id)
        .expect("PDF/UA-1 fixture Form structure element")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture Form structure element dictionary");
    match case {
        "allowed" => {}
        "invalid" => structure_form.set("K", vec![child, Object::Integer(0)]),
        "role_attribute" => {
            structure_form.set("K", vec![child, Object::Integer(0)]);
            structure_form.set(
                "A",
                dictionary! {
                    "O" => "PrintField",
                    "Role" => "PushButton",
                },
            );
        }
        _ => panic!("unknown PDF/UA-1 rule 7.18.4-2 fixture case {case}"),
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 Form child fixture");
    bytes
}

pub fn pdfua1_rule_7_18_5_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_18_1_1_fixture("valid"))
        .expect("load PDF/UA-1 Link-tag fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let pages_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"Pages")
        .expect("PDF/UA-1 fixture pages")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture pages");
    let page_id = document
        .get_object(pages_id)
        .expect("PDF/UA-1 fixture pages object")
        .as_dict()
        .expect("PDF/UA-1 fixture pages dictionary")
        .get(b"Kids")
        .expect("PDF/UA-1 fixture page kids")
        .as_array()
        .expect("PDF/UA-1 fixture page kids array")
        .first()
        .expect("PDF/UA-1 fixture page")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture page");
    let annotation_id = document
        .get_object(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict()
        .expect("PDF/UA-1 fixture page dictionary")
        .get(b"Annots")
        .expect("PDF/UA-1 fixture annotations")
        .as_array()
        .expect("PDF/UA-1 fixture annotations array")
        .first()
        .expect("PDF/UA-1 fixture annotation")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture annotation");
    let structure_link_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"ParentTree")
        .expect("PDF/UA-1 fixture parent tree")
        .as_dict()
        .expect("PDF/UA-1 fixture parent tree dictionary")
        .get(b"Nums")
        .expect("PDF/UA-1 fixture parent tree numbers")
        .as_array()
        .expect("PDF/UA-1 fixture parent tree numbers array")
        .chunks(2)
        .find_map(|pair| {
            (pair.first()?.as_i64().ok()? == 100)
                .then(|| pair.get(1)?.as_reference().ok())
                .flatten()
        })
        .expect("PDF/UA-1 fixture Link structure element");
    document
        .get_object_mut(annotation_id)
        .expect("PDF/UA-1 fixture annotation")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture annotation dictionary")
        .set("Subtype", "Link");
    document
        .get_object_mut(structure_link_id)
        .expect("PDF/UA-1 fixture Link structure element")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture Link structure element dictionary")
        .set("S", "Link");

    match case {
        "allowed" => {}
        "role_mapped" => {
            document
                .get_object_mut(struct_tree_root_id)
                .expect("PDF/UA-1 fixture structure tree root")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture structure tree root dictionary")
                .set("RoleMap", dictionary! {"CustomLink" => "Link"});
            document
                .get_object_mut(structure_link_id)
                .expect("PDF/UA-1 fixture Link structure element")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture Link structure element dictionary")
                .set("S", "CustomLink");
        }
        "hidden" => {
            document
                .get_object_mut(annotation_id)
                .expect("PDF/UA-1 fixture annotation")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture annotation dictionary")
                .set("F", 2);
            document
                .get_object_mut(structure_link_id)
                .expect("PDF/UA-1 fixture Link structure element")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture Link structure element dictionary")
                .set("S", "P");
        }
        "outside_crop_box" => {
            document
                .get_object_mut(annotation_id)
                .expect("PDF/UA-1 fixture annotation")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture annotation dictionary")
                .set(
                    "Rect",
                    vec![
                        Object::Integer(-10),
                        Object::Integer(-10),
                        Object::Integer(-1),
                        Object::Integer(-1),
                    ],
                );
            document
                .get_object_mut(structure_link_id)
                .expect("PDF/UA-1 fixture Link structure element")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture Link structure element dictionary")
                .set("S", "P");
        }
        "not_nested" => {
            document
                .get_object_mut(structure_link_id)
                .expect("PDF/UA-1 fixture Link structure element")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture Link structure element dictionary")
                .set("S", "P");
        }
        _ => panic!("unknown PDF/UA-1 rule 7.18.5-1 fixture case {case}"),
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 Link-tag fixture");
    bytes
}

pub fn pdfua1_rule_7_18_5_2_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_18_5_1_fixture("allowed"))
        .expect("load PDF/UA-1 Link-contents fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let pages_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"Pages")
        .expect("PDF/UA-1 fixture pages")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture pages");
    let page_id = document
        .get_object(pages_id)
        .expect("PDF/UA-1 fixture pages object")
        .as_dict()
        .expect("PDF/UA-1 fixture pages dictionary")
        .get(b"Kids")
        .expect("PDF/UA-1 fixture page kids")
        .as_array()
        .expect("PDF/UA-1 fixture page kids array")
        .first()
        .expect("PDF/UA-1 fixture page")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture page");
    let annotation_id = document
        .get_object(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict()
        .expect("PDF/UA-1 fixture page dictionary")
        .get(b"Annots")
        .expect("PDF/UA-1 fixture annotations")
        .as_array()
        .expect("PDF/UA-1 fixture annotations array")
        .first()
        .expect("PDF/UA-1 fixture annotation")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture annotation");
    let annotation = document
        .get_object_mut(annotation_id)
        .expect("PDF/UA-1 fixture annotation")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture annotation dictionary");
    match case {
        "allowed" => {}
        "missing" => {
            annotation.remove(b"Contents");
        }
        "empty_contents" => {
            annotation.set("Contents", Object::string_literal(""));
        }
        "hidden" => {
            annotation.set("F", 2);
            annotation.remove(b"Contents");
        }
        "outside_crop_box" => {
            annotation.set(
                "Rect",
                vec![
                    Object::Integer(-10),
                    Object::Integer(-10),
                    Object::Integer(-1),
                    Object::Integer(-1),
                ],
            );
            annotation.remove(b"Contents");
        }
        _ => panic!("unknown PDF/UA-1 rule 7.18.5-2 fixture case {case}"),
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 Link-contents fixture");
    bytes
}

pub fn pdfua1_rule_7_18_8_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_18_1_1_fixture("valid"))
        .expect("load PDF/UA-1 PrinterMark fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let pages_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"Pages")
        .expect("PDF/UA-1 fixture pages")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture pages");
    let page_id = document
        .get_object(pages_id)
        .expect("PDF/UA-1 fixture pages object")
        .as_dict()
        .expect("PDF/UA-1 fixture pages dictionary")
        .get(b"Kids")
        .expect("PDF/UA-1 fixture page kids")
        .as_array()
        .expect("PDF/UA-1 fixture page kids array")
        .first()
        .expect("PDF/UA-1 fixture page")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture page");
    let annotation_id = document
        .get_object(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict()
        .expect("PDF/UA-1 fixture page dictionary")
        .get(b"Annots")
        .expect("PDF/UA-1 fixture annotations")
        .as_array()
        .expect("PDF/UA-1 fixture annotations array")
        .first()
        .expect("PDF/UA-1 fixture annotation")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture annotation");
    document
        .get_object_mut(annotation_id)
        .expect("PDF/UA-1 fixture annotation")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture annotation dictionary")
        .set("Subtype", "PrinterMark");
    match case {
        "allowed" => {
            document
                .get_object_mut(annotation_id)
                .expect("PDF/UA-1 fixture annotation")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture annotation dictionary")
                .remove(b"StructParent");
        }
        "hidden" => {
            document
                .get_object_mut(annotation_id)
                .expect("PDF/UA-1 fixture annotation")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture annotation dictionary")
                .set("F", 2);
        }
        "outside_crop_box" => {
            document
                .get_object_mut(annotation_id)
                .expect("PDF/UA-1 fixture annotation")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture annotation dictionary")
                .set(
                    "Rect",
                    vec![
                        Object::Integer(-10),
                        Object::Integer(-10),
                        Object::Integer(-1),
                        Object::Integer(-1),
                    ],
                );
        }
        "included" => {}
        _ => panic!("unknown PDF/UA-1 rule 7.18.8-1 fixture case {case}"),
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 PrinterMark fixture");
    bytes
}

pub fn pdfua1_rule_7_18_6_2_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_18_1_1_fixture("valid"))
        .expect("load PDF/UA-1 media clip fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let pages_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"Pages")
        .expect("PDF/UA-1 fixture pages")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture pages");
    let page_id = document
        .get_object(pages_id)
        .expect("PDF/UA-1 fixture pages object")
        .as_dict()
        .expect("PDF/UA-1 fixture pages dictionary")
        .get(b"Kids")
        .expect("PDF/UA-1 fixture page kids")
        .as_array()
        .expect("PDF/UA-1 fixture page kids array")
        .first()
        .expect("PDF/UA-1 fixture page")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture page");
    let annotation_id = document
        .get_object(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict()
        .expect("PDF/UA-1 fixture page dictionary")
        .get(b"Annots")
        .expect("PDF/UA-1 fixture annotations")
        .as_array()
        .expect("PDF/UA-1 fixture annotations array")
        .first()
        .expect("PDF/UA-1 fixture annotation")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture annotation");
    let media_clip_id = document.add_object(dictionary! {
        "Type" => "MediaClip",
        "S" => "MCD",
        "D" => Object::string_literal("video.mp4"),
        "CT" => Object::string_literal("video/mp4"),
        "Alt" => vec![
            Object::string_literal("en"),
            Object::string_literal("Video"),
        ],
    });
    match case {
        "allowed" | "missing_alt" | "invalid_alt" => {}
        "missing_ct" => {
            document
                .get_object_mut(media_clip_id)
                .expect("PDF/UA-1 fixture media clip")
                .as_dict_mut()
                .expect("PDF/UA-1 fixture media clip dictionary")
                .remove(b"CT");
        }
        _ => panic!("unknown PDF/UA-1 rule 7.18.6.2 fixture case {case}"),
    }
    if case == "missing_alt" {
        document
            .get_object_mut(media_clip_id)
            .expect("PDF/UA-1 fixture media clip")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture media clip dictionary")
            .remove(b"Alt");
    } else if case == "invalid_alt" {
        document
            .get_object_mut(media_clip_id)
            .expect("PDF/UA-1 fixture media clip")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture media clip dictionary")
            .set("Alt", vec![Object::string_literal("en")]);
    }
    let rendition_id = document.add_object(dictionary! {
        "Type" => "Rendition",
        "S" => "MR",
        "C" => media_clip_id,
    });
    let action_id = document.add_object(dictionary! {
        "Type" => "Action",
        "S" => "Rendition",
        "R" => rendition_id,
        "AN" => annotation_id,
    });
    let annotation = document
        .get_object_mut(annotation_id)
        .expect("PDF/UA-1 fixture annotation")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture annotation dictionary");
    annotation.set("Subtype", "Screen");
    annotation.set("A", action_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 media clip fixture");
    bytes
}

pub fn pdfua1_rule_7_2_24_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 annotation-language fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let pages_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"Pages")
        .expect("PDF/UA-1 fixture pages")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture pages");
    let page_id = document
        .get_object(pages_id)
        .expect("PDF/UA-1 fixture pages object")
        .as_dict()
        .expect("PDF/UA-1 fixture pages dictionary")
        .get(b"Kids")
        .expect("PDF/UA-1 fixture page kids")
        .as_array()
        .expect("PDF/UA-1 fixture page kids array")
        .first()
        .expect("PDF/UA-1 fixture page")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture page");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let annotation = dictionary! {
        "Type" => "Annot",
        "Subtype" => "Text",
        "Rect" => vec![0.into(), 0.into(), 10.into(), 10.into()],
        "Contents" => Object::string_literal("Annotation"),
        "StructParent" => 1,
    };
    let annotation_id = document.add_object(annotation);
    let structure_annotation_id = document.new_object_id();
    let mut structure_annotation = dictionary! {
        "Type" => "StructElem",
        "S" => "Annot",
        "P" => struct_tree_root_id,
        "K" => annotation_id,
        "Pg" => page_id,
    };
    let catalog_language_present = match case {
        "annotation_language_present" => {
            structure_annotation.set("Lang", Object::string_literal("en"));
            false
        }
        "catalog_language_present" => true,
        "language_missing" => false,
        _ => panic!("unknown PDF/UA-1 rule 7.2-24 fixture case {case}"),
    };
    document.objects.insert(
        structure_annotation_id,
        Object::Dictionary(structure_annotation),
    );
    let catalog = document
        .get_object_mut(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture catalog dictionary");
    catalog.remove(b"Outlines");
    if catalog_language_present {
        catalog.set("Lang", Object::string_literal("en"));
    } else {
        catalog.remove(b"Lang");
    }
    let structure_tree_root = document
        .get_object_mut(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture structure tree root dictionary");
    structure_tree_root
        .get_mut(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array_mut()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .push(Object::Reference(structure_annotation_id));
    structure_tree_root
        .get_mut(b"ParentTree")
        .expect("PDF/UA-1 fixture parent tree")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture parent tree dictionary")
        .get_mut(b"Nums")
        .expect("PDF/UA-1 fixture parent tree numbers")
        .as_array_mut()
        .expect("PDF/UA-1 fixture parent tree numbers array")
        .extend([
            Object::Integer(1),
            Object::Reference(structure_annotation_id),
        ]);
    let page = document
        .get_object_mut(page_id)
        .expect("PDF/UA-1 fixture page")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture page dictionary");
    page.set("Tabs", "S");
    page.set("Annots", vec![Object::Reference(annotation_id)]);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 annotation-language fixture");
    bytes
}

fn pdfua1_rule_7_2_text_language_fixture(case: &str, attribute: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 text-language fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let top_level_structure_element_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root object")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .first()
        .expect("PDF/UA-1 fixture structure tree root first kid")
        .as_reference()
        .expect("indirect PDF/UA-1 top-level structure element");
    let structure_element_id = document
        .get_object(top_level_structure_element_id)
        .expect("PDF/UA-1 fixture top-level structure element")
        .as_dict()
        .expect("PDF/UA-1 fixture top-level structure element dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture top-level structure element kids")
        .as_array()
        .expect("PDF/UA-1 fixture top-level structure element kids array")
        .first()
        .expect("PDF/UA-1 fixture top-level structure element first kid")
        .as_reference()
        .expect("indirect PDF/UA-1 structure element");
    let catalog = document
        .get_object_mut(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture catalog dictionary");
    catalog.remove(b"Lang");
    catalog.remove(b"Outlines");
    let structure_element = document
        .get_object_mut(structure_element_id)
        .expect("PDF/UA-1 fixture structure element")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture structure element dictionary");
    structure_element.set(attribute, Object::string_literal("Heading"));
    if case == "language_present" {
        structure_element.set("Lang", Object::string_literal("en"));
    } else if case != "language_missing" {
        panic!("unknown PDF/UA-1 text-language fixture case {case}");
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 text-language fixture");
    bytes
}

pub fn pdfua1_rule_7_1_5_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 rule 7.1-5 fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let struct_tree_root_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"StructTreeRoot")
        .expect("PDF/UA-1 fixture structure tree root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture structure tree root");
    let structure_element_id = document
        .get_object(struct_tree_root_id)
        .expect("PDF/UA-1 fixture structure tree root object")
        .as_dict()
        .expect("PDF/UA-1 fixture structure tree root dictionary")
        .get(b"K")
        .expect("PDF/UA-1 fixture structure tree root kids")
        .as_array()
        .expect("PDF/UA-1 fixture structure tree root kids array")
        .first()
        .expect("PDF/UA-1 fixture structure tree root first kid")
        .as_reference()
        .expect("indirect PDF/UA-1 top-level structure element");
    document
        .get_object_mut(structure_element_id)
        .expect("PDF/UA-1 fixture structure element")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture structure element dictionary")
        .set("S", "CustomHeading");
    match case {
        "indirect_mapping" => document
            .get_object_mut(struct_tree_root_id)
            .expect("PDF/UA-1 fixture structure tree root object")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture structure tree root dictionary")
            .set(
                "RoleMap",
                dictionary! {
                    "CustomHeading" => "IntermediateHeading",
                    "IntermediateHeading" => "H1",
                },
            ),
        "unmapped" => {}
        _ => panic!("unknown PDF/UA-1 rule 7.1-5 fixture case {case}"),
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.1-5 fixture");
    bytes
}

pub fn pdfua1_rule_7_1_6_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_5_fixture("indirect_mapping"))
        .expect("load PDF/UA-1 rule 7.1-6 fixture");
    if case == "circular_mapping" {
        let root_id = document
            .trailer
            .get(b"Root")
            .expect("PDF/UA-1 fixture root")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture root");
        let struct_tree_root_id = document
            .get_object(root_id)
            .expect("PDF/UA-1 fixture catalog")
            .as_dict()
            .expect("PDF/UA-1 fixture catalog dictionary")
            .get(b"StructTreeRoot")
            .expect("PDF/UA-1 fixture structure tree root")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture structure tree root");
        document
            .get_object_mut(struct_tree_root_id)
            .expect("PDF/UA-1 fixture structure tree root object")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture structure tree root dictionary")
            .set(
                "RoleMap",
                dictionary! {
                    "CustomHeading" => "IntermediateHeading",
                    "IntermediateHeading" => "CustomHeading",
                },
            );
    } else if case != "acyclic_mapping" {
        panic!("unknown PDF/UA-1 rule 7.1-6 fixture case {case}");
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.1-6 fixture");
    bytes
}

pub fn pdfua1_rule_7_1_7_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 rule 7.1-7 fixture");
    if case == "standard_remapped" {
        let root_id = document
            .trailer
            .get(b"Root")
            .expect("PDF/UA-1 fixture root")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture root");
        let struct_tree_root_id = document
            .get_object(root_id)
            .expect("PDF/UA-1 fixture catalog")
            .as_dict()
            .expect("PDF/UA-1 fixture catalog dictionary")
            .get(b"StructTreeRoot")
            .expect("PDF/UA-1 fixture structure tree root")
            .as_reference()
            .expect("indirect PDF/UA-1 fixture structure tree root");
        let structure_element_id = document
            .get_object(struct_tree_root_id)
            .expect("PDF/UA-1 fixture structure tree root object")
            .as_dict()
            .expect("PDF/UA-1 fixture structure tree root dictionary")
            .get(b"K")
            .expect("PDF/UA-1 fixture structure tree root kids")
            .as_array()
            .expect("PDF/UA-1 fixture structure tree root kids array")
            .first()
            .expect("PDF/UA-1 fixture structure tree root first kid")
            .as_reference()
            .expect("indirect PDF/UA-1 top-level structure element");
        document
            .get_object_mut(structure_element_id)
            .expect("PDF/UA-1 fixture structure element")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture structure element dictionary")
            .set("S", "P");
        document
            .get_object_mut(struct_tree_root_id)
            .expect("PDF/UA-1 fixture structure tree root object")
            .as_dict_mut()
            .expect("PDF/UA-1 fixture structure tree root dictionary")
            .set("RoleMap", dictionary! { "P" => "Span" });
    } else if case != "standard_unmapped" {
        panic!("unknown PDF/UA-1 rule 7.1-7 fixture case {case}");
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.1-7 fixture");
    bytes
}

pub fn pdfua1_rule_7_1_1_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 rule 7.1-1 fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let pages_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"Pages")
        .expect("PDF/UA-1 fixture pages")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture pages");
    let page_id = document
        .get_object(pages_id)
        .expect("PDF/UA-1 fixture pages object")
        .as_dict()
        .expect("PDF/UA-1 fixture pages dictionary")
        .get(b"Kids")
        .expect("PDF/UA-1 fixture page kids")
        .as_array()
        .expect("PDF/UA-1 fixture page kids array")
        .first()
        .expect("PDF/UA-1 fixture page")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture page");
    let content = match case {
        "artifact_outside_tagged_content" => b"/Artifact BMC\nEMC\n".as_slice(),
        "artifact_inside_tagged_content" => {
            b"/P <</MCID 0>> BDC\n/Artifact BMC\nEMC\nEMC\n".as_slice()
        }
        _ => panic!("unknown PDF/UA-1 rule 7.1-1 fixture case {case}"),
    };
    let content_id = document.add_object(Stream::new(Dictionary::new(), content.to_vec()));
    document
        .get_object_mut(page_id)
        .expect("PDF/UA-1 fixture page object")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture page dictionary")
        .set("Contents", content_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.1-1 fixture");
    bytes
}

pub fn pdfua1_rule_7_1_2_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 rule 7.1-2 fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let pages_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"Pages")
        .expect("PDF/UA-1 fixture pages")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture pages");
    let page_id = document
        .get_object(pages_id)
        .expect("PDF/UA-1 fixture pages object")
        .as_dict()
        .expect("PDF/UA-1 fixture pages dictionary")
        .get(b"Kids")
        .expect("PDF/UA-1 fixture page kids")
        .as_array()
        .expect("PDF/UA-1 fixture page kids array")
        .first()
        .expect("PDF/UA-1 fixture page")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture page");
    let content = match case {
        "tagged_outside_artifact" => b"/Artifact BMC\nEMC\n/P <</MCID 0>> BDC\nEMC\n".as_slice(),
        "tagged_inside_artifact" => b"/Artifact BMC\n/P <</MCID 0>> BDC\nEMC\nEMC\n".as_slice(),
        _ => panic!("unknown PDF/UA-1 rule 7.1-2 fixture case {case}"),
    };
    let content_id = document.add_object(Stream::new(Dictionary::new(), content.to_vec()));
    document
        .get_object_mut(page_id)
        .expect("PDF/UA-1 fixture page object")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture page dictionary")
        .set("Contents", content_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.1-2 fixture");
    bytes
}

pub fn pdfua1_rule_7_1_3_fixture(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(&pdfua1_rule_7_1_12_fixture("present"))
        .expect("load PDF/UA-1 rule 7.1-3 fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("PDF/UA-1 fixture root")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture root");
    let pages_id = document
        .get_object(root_id)
        .expect("PDF/UA-1 fixture catalog")
        .as_dict()
        .expect("PDF/UA-1 fixture catalog dictionary")
        .get(b"Pages")
        .expect("PDF/UA-1 fixture pages")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture pages");
    let page_id = document
        .get_object(pages_id)
        .expect("PDF/UA-1 fixture pages object")
        .as_dict()
        .expect("PDF/UA-1 fixture pages dictionary")
        .get(b"Kids")
        .expect("PDF/UA-1 fixture page kids")
        .as_array()
        .expect("PDF/UA-1 fixture page kids array")
        .first()
        .expect("PDF/UA-1 fixture page")
        .as_reference()
        .expect("indirect PDF/UA-1 fixture page");
    let path = b"0 0 m 10 10 l S\n";
    let content = match case {
        "artifact" => [b"/Artifact BMC\n".as_slice(), path, b"EMC\n"].concat(),
        "tagged" => [b"/P <</MCID 0>> BDC\n".as_slice(), path, b"EMC\n"].concat(),
        "untagged" => path.to_vec(),
        _ => panic!("unknown PDF/UA-1 rule 7.1-3 fixture case {case}"),
    };
    let content_id = document.add_object(Stream::new(Dictionary::new(), content));
    document
        .get_object_mut(page_id)
        .expect("PDF/UA-1 fixture page object")
        .as_dict_mut()
        .expect("PDF/UA-1 fixture page dictionary")
        .set("Contents", content_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/UA-1 rule 7.1-3 fixture");
    bytes
}

pub fn output_intent_fixture(case: &str) -> Vec<u8> {
    let mut document = pdf_document();
    let pages_id = document.new_object_id();
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });
    wrap_pages(&mut document, pages_id, page_id);
    let metadata_id = standard_metadata_stream(&mut document);
    let info_id = document.add_object(complete_info());

    let rgb = icc_header(*b"mntr", *b"RGB ", 2, 1);
    let output_intents = match case {
        "baseline" => single_profile_intent(&mut document, rgb.clone(), Some("GTS_PDFA1")),
        "no_output_intents" => None,
        "wrong_type_array" => Some(42.into()),
        "empty_array" => Some(Object::Array(Vec::new())),
        "non_dictionary_entries" => {
            let number_id = document.add_object(Object::Integer(7));
            Some(Object::Array(vec![
                5.into(),
                Object::Reference(number_id),
                Object::Null,
            ]))
        }
        "direct_intent_dictionary" => Some(Object::Array(vec![Object::Dictionary(
            output_intent_dictionary(
                Some(profile_reference(&mut document, rgb.clone())),
                Some("GTS_PDFA1"),
            ),
        )])),
        "missing_s" => single_profile_intent(&mut document, rgb.clone(), None),
        "wrong_s" => single_profile_intent(&mut document, rgb.clone(), Some("GTS_PDFX")),
        "pdfx_with_dest_output_profile_ref" => {
            let profile = profile_reference(&mut document, rgb.clone());
            let mut intent = output_intent_dictionary(Some(profile), Some("GTS_PDFX"));
            intent.set("DestOutputProfileRef", Dictionary::new());
            Some(Object::Array(vec![Object::Reference(
                document.add_object(intent),
            )]))
        }
        "missing_dest_output_profile" => single_intent(&mut document, None, Some("GTS_PDFA1")),
        "direct_wrong_type_profile" => {
            single_intent(&mut document, Some(7.into()), Some("GTS_PDFA1"))
        }
        "indirect_wrong_type_profile" => {
            let wrong_id = document.add_object(dictionary! {"Not" => "AStream"});
            single_intent(
                &mut document,
                Some(Object::Reference(wrong_id)),
                Some("GTS_PDFA1"),
            )
        }
        "truncated_profile" => single_profile_intent(&mut document, vec![0; 19], Some("GTS_PDFA1")),
        "class_prtr" => single_profile_intent(
            &mut document,
            icc_header(*b"prtr", *b"RGB ", 2, 1),
            Some("GTS_PDFA1"),
        ),
        "class_scnr" => single_profile_intent(
            &mut document,
            icc_header(*b"scnr", *b"RGB ", 2, 1),
            Some("GTS_PDFA1"),
        ),
        "color_cmyk" => single_profile_intent(
            &mut document,
            icc_header(*b"mntr", *b"CMYK", 2, 1),
            Some("GTS_PDFA1"),
        ),
        "color_gray" => single_profile_intent(
            &mut document,
            icc_header(*b"mntr", *b"GRAY", 2, 1),
            Some("GTS_PDFA1"),
        ),
        "color_lab" => single_profile_intent(
            &mut document,
            icc_header(*b"mntr", *b"Lab ", 2, 1),
            Some("GTS_PDFA1"),
        ),
        "version_2_15" => single_profile_intent(
            &mut document,
            icc_header(*b"mntr", *b"RGB ", 2, 15),
            Some("GTS_PDFA1"),
        ),
        "version_3" => single_profile_intent(
            &mut document,
            icc_header(*b"mntr", *b"RGB ", 3, 0),
            Some("GTS_PDFA1"),
        ),
        "large_compressed_profile" => {
            let mut bytes = rgb.clone();
            bytes.resize(4096, 0);
            let profile = compressed_profile_reference(&mut document, bytes);
            single_intent(&mut document, Some(profile), Some("GTS_PDFA1"))
        }
        "two_shared_indirect_profiles" => {
            let profile = profile_reference(&mut document, rgb.clone());
            two_intents(&mut document, profile.clone(), profile)
        }
        "two_shared_invalid_profiles" => {
            let profile = profile_reference(&mut document, icc_header(*b"scnr", *b"RGB ", 2, 1));
            two_intents(&mut document, profile.clone(), profile)
        }
        "two_identical_indirect_profiles" => {
            let first = profile_reference(&mut document, rgb.clone());
            let second = profile_reference(&mut document, rgb.clone());
            two_intents(&mut document, first, second)
        }
        "two_different_indirect_profiles" => {
            let first = profile_reference(&mut document, rgb.clone());
            let second = profile_reference(&mut document, icc_header(*b"mntr", *b"CMYK", 2, 1));
            two_intents(&mut document, first, second)
        }
        "one_profile_one_missing" => {
            let profile = profile_reference(&mut document, rgb.clone());
            let first =
                document.add_object(output_intent_dictionary(Some(profile), Some("GTS_PDFA1")));
            let second = document.add_object(output_intent_dictionary(None, Some("GTS_PDFA1")));
            Some(Object::Array(vec![
                Object::Reference(first),
                Object::Reference(second),
            ]))
        }
        "two_same_wrong_type_indirect_profiles" => {
            let wrong = document.add_object(dictionary! {"Not" => "AStream"});
            two_intents(
                &mut document,
                Object::Reference(wrong),
                Object::Reference(wrong),
            )
        }
        "two_different_wrong_type_indirect_profiles" => {
            let first = document.add_object(dictionary! {"Not" => "AStream"});
            let second = document.add_object(dictionary! {"StillNot" => "AStream"});
            two_intents(
                &mut document,
                Object::Reference(first),
                Object::Reference(second),
            )
        }
        _ => panic!("unknown output-intent fixture case {case}"),
    };

    let mut catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
    };
    if let Some(output_intents) = output_intents {
        catalog.set("OutputIntents", output_intents);
    }
    let catalog_id = document.add_object(catalog);
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save output-intent fixture");
    bytes
}

pub fn icc_based_fixture(case: &str) -> Vec<u8> {
    let mut document = pdf_document();
    let pages_id = document.new_object_id();
    let valid = icc_header(*b"mntr", *b"RGB ", 2, 1);
    let (class, color_space, version_major, version_minor) = match case {
        "class_prtr" => (*b"prtr", *b"RGB ", 2, 1),
        "class_mntr" => (*b"mntr", *b"RGB ", 2, 1),
        "class_scnr" => (*b"scnr", *b"RGB ", 2, 1),
        "class_spac" => (*b"spac", *b"RGB ", 2, 1),
        "color_rgb" => (*b"mntr", *b"RGB ", 2, 1),
        "color_cmyk" => (*b"mntr", *b"CMYK", 2, 1),
        "color_gray" => (*b"mntr", *b"GRAY", 2, 1),
        "color_lab" => (*b"mntr", *b"Lab ", 2, 1),
        "version_2_15" => (*b"mntr", *b"RGB ", 2, 15),
        "invalid_class"
        | "direct_profile"
        | "repeated_shared_invalid"
        | "two_invalid_profiles"
        | "form_used"
        | "form_unused_resource"
        | "form_unreferenced"
        | "nested_form_used"
        | "cyclic_form"
        | "image_used"
        | "image_unused_resource"
        | "image_unreferenced"
        | "image_mask_ignores_color_space"
        | "image_smask_used"
        | "image_mask_image_used"
        | "image_alternate_used"
        | "unused_resource"
        | "default_gray"
        | "default_rgb"
        | "default_cmyk"
        | "unused_default"
        | "form_parent_fallback"
        | "nested_form_page_fallback"
        | "inline_image_used"
        | "shading_used"
        | "indexed_base_used" => (*b"link", *b"RGB ", 2, 1),
        "invalid_color_space" => (*b"mntr", *b"XYZ ", 2, 1),
        "version_3" => (*b"mntr", *b"RGB ", 3, 0),
        _ => (*b"mntr", *b"RGB ", 2, 1),
    };
    let selected_bytes = if case == "truncated_profile" {
        vec![0; 19]
    } else {
        icc_header(class, color_space, version_major, version_minor)
    };
    let selected_profile = if case == "undecodable_profile" {
        let mut stream = Stream::new(dictionary! {"N" => 3}, b"not deflate data".to_vec());
        stream.dict.set("Filter", "FlateDecode");
        Object::Reference(document.add_object(stream))
    } else if case == "large_compressed_profile" {
        let mut bytes = valid.clone();
        bytes.resize(4096, 0);
        compressed_profile_reference(&mut document, bytes)
    } else if matches!(case, "missing_n" | "wrong_n" | "non_integer_n") {
        let mut dictionary = Dictionary::new();
        if case == "wrong_n" {
            dictionary.set("N", 4);
        } else if case == "non_integer_n" {
            dictionary.set("N", Object::Name(b"Three".to_vec()));
        }
        Object::Reference(document.add_object(Stream::new(dictionary, selected_bytes)))
    } else {
        profile_reference(&mut document, selected_bytes)
    };
    let indirect_space = Object::Array(vec![
        Object::Name(b"ICCBased".to_vec()),
        selected_profile.clone(),
    ]);
    let direct_space = Object::Array(vec![
        Object::Name(b"ICCBased".to_vec()),
        Object::Stream(profile_stream(icc_header(*b"link", *b"RGB ", 2, 1))),
    ]);

    let mut page_resources = Dictionary::new();
    let mut page_contents = b"/CS1 CS\n".to_vec();
    match case {
        "baseline"
        | "class_prtr"
        | "class_mntr"
        | "class_scnr"
        | "class_spac"
        | "color_rgb"
        | "color_cmyk"
        | "color_gray"
        | "color_lab"
        | "version_2_15"
        | "invalid_class"
        | "invalid_color_space"
        | "version_3"
        | "truncated_profile"
        | "undecodable_profile"
        | "large_compressed_profile"
        | "missing_n"
        | "wrong_n"
        | "non_integer_n"
        | "inherited_resources" => {
            page_resources.set("ColorSpace", dictionary! {"CS1" => indirect_space});
        }
        "direct_profile" => {
            page_resources.set("ColorSpace", dictionary! {"CS1" => direct_space});
        }
        "unused_resource" => {
            page_resources.set("ColorSpace", dictionary! {"CS1" => indirect_space});
            page_contents.clear();
        }
        "default_gray" | "default_rgb" | "default_cmyk" | "unused_default" => {
            let (name, content) = match case {
                "default_gray" => ("DefaultGray", b"0 g\n0 G\n".as_slice()),
                "default_rgb" => ("DefaultRGB", b"0 0 0 rg\n0 0 0 RG\n".as_slice()),
                "default_cmyk" => ("DefaultCMYK", b"0 0 0 0 k\n0 0 0 0 K\n".as_slice()),
                _ => ("DefaultRGB", b"".as_slice()),
            };
            page_resources.set(
                "ColorSpace",
                Dictionary::from_iter([(name.as_bytes(), indirect_space)]),
            );
            page_contents = content.to_vec();
        }
        "missing_profile" => {
            page_resources.set(
                "ColorSpace",
                dictionary! {
                    "CS1" => Object::Array(vec![Object::Name(b"ICCBased".to_vec())]),
                },
            );
        }
        "wrong_profile_type" => {
            page_resources.set(
                "ColorSpace",
                dictionary! {
                    "CS1" => Object::Array(vec![
                        Object::Name(b"ICCBased".to_vec()),
                        Object::Integer(7),
                    ]),
                },
            );
        }
        "repeated_shared_valid" | "repeated_shared_invalid" => {
            page_resources.set(
                "ColorSpace",
                dictionary! {
                    "CS1" => indirect_space.clone(),
                    "CS2" => indirect_space,
                },
            );
            page_contents = b"/CS1 CS\n/CS2 cs\n".to_vec();
        }
        "two_invalid_profiles" => {
            let second = profile_reference(&mut document, icc_header(*b"mntr", *b"XYZ ", 2, 1));
            page_resources.set(
                "ColorSpace",
                dictionary! {
                    "CS1" => indirect_space,
                    "CS2" => Object::Array(vec![
                        Object::Name(b"ICCBased".to_vec()),
                        second,
                    ]),
                },
            );
            page_contents = b"/CS1 CS\n/CS2 cs\n".to_vec();
        }
        "form_used" | "form_unused_resource" | "form_unreferenced" => {
            let form_contents = if case == "form_unused_resource" {
                Vec::new()
            } else {
                b"/CS1 CS\n".to_vec()
            };
            let form = Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Resources" => dictionary! {
                        "ColorSpace" => dictionary! {"CS1" => indirect_space},
                    },
                },
                form_contents,
            );
            let form_id = document.add_object(form);
            if case != "form_unreferenced" {
                page_resources.set("XObject", dictionary! {"Fm" => form_id});
            }
            page_contents = if case == "form_used" || case == "form_unused_resource" {
                b"/Fm Do\n".to_vec()
            } else {
                Vec::new()
            };
        }
        "nested_form_used" => {
            let inner_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Resources" => dictionary! {
                        "ColorSpace" => dictionary! {"CS1" => indirect_space},
                    },
                },
                b"/CS1 CS\n".to_vec(),
            ));
            let outer_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Resources" => dictionary! {
                        "XObject" => dictionary! {"Inner" => inner_id},
                    },
                },
                b"/Inner Do\n".to_vec(),
            ));
            page_resources.set("XObject", dictionary! {"Outer" => outer_id});
            page_contents = b"/Outer Do\n".to_vec();
        }
        "form_parent_fallback" => {
            let form_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Resources" => dictionary! {
                        "XObject" => Dictionary::new(),
                    },
                },
                b"/CS1 CS\n".to_vec(),
            ));
            page_resources.set("ColorSpace", dictionary! {"CS1" => indirect_space});
            page_resources.set("XObject", dictionary! {"Fm" => form_id});
            page_contents = b"/Fm Do\n".to_vec();
        }
        "nested_form_page_fallback" => {
            let inner_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Resources" => Dictionary::new(),
                },
                b"/CS1 CS\n".to_vec(),
            ));
            let outer_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Resources" => dictionary! {
                        "XObject" => dictionary! {"Inner" => inner_id},
                    },
                },
                b"/Inner Do\n".to_vec(),
            ));
            page_resources.set("ColorSpace", dictionary! {"CS1" => indirect_space});
            page_resources.set("XObject", dictionary! {"Outer" => outer_id});
            page_contents = b"/Outer Do\n".to_vec();
        }
        "cyclic_form" => {
            let form_id = document.new_object_id();
            document.objects.insert(
                form_id,
                Object::Stream(Stream::new(
                    dictionary! {
                        "Type" => "XObject",
                        "Subtype" => "Form",
                        "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                        "Resources" => dictionary! {
                            "ColorSpace" => dictionary! {"CS1" => indirect_space},
                            "XObject" => dictionary! {"Self" => form_id},
                        },
                    },
                    b"/CS1 CS\n/Self Do\n".to_vec(),
                )),
            );
            page_resources.set("XObject", dictionary! {"Fm" => form_id});
            page_contents = b"/Fm Do\n".to_vec();
        }
        "image_used" | "image_unused_resource" | "image_unreferenced" => {
            let image = Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 8,
                    "ColorSpace" => indirect_space,
                },
                vec![0, 0, 0],
            );
            let image_id = document.add_object(image);
            if case != "image_unreferenced" {
                page_resources.set("XObject", dictionary! {"Im" => image_id});
            }
            page_contents = if case == "image_used" {
                b"/Im Do\n".to_vec()
            } else {
                Vec::new()
            };
        }
        "image_mask_ignores_color_space" => {
            let image_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 1,
                    "ImageMask" => true,
                    "ColorSpace" => indirect_space,
                },
                vec![0],
            ));
            page_resources.set("XObject", dictionary! {"Im" => image_id});
            page_contents = b"/Im Do\n".to_vec();
        }
        "image_smask_used" | "image_mask_image_used" | "image_alternate_used" => {
            let linked_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 8,
                    "ColorSpace" => indirect_space,
                },
                vec![0, 0, 0],
            ));
            let mut primary = dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 1,
                "Height" => 1,
                "BitsPerComponent" => 8,
                "ColorSpace" => "DeviceRGB",
            };
            match case {
                "image_smask_used" => primary.set("SMask", linked_id),
                "image_mask_image_used" => primary.set("Mask", linked_id),
                _ => primary.set(
                    "Alternates",
                    vec![Object::Dictionary(dictionary! {
                        "Image" => linked_id,
                        "DefaultForPrinting" => true,
                    })],
                ),
            }
            let primary_id = document.add_object(Stream::new(primary, vec![0, 0, 0]));
            page_resources.set("XObject", dictionary! {"Im" => primary_id});
            page_contents = b"/Im Do\n".to_vec();
        }
        "inline_image_used" => {
            page_resources.set("ColorSpace", dictionary! {"CS1" => indirect_space});
            page_contents = b"q\nBI /W 1 /H 1 /BPC 8 /CS /CS1 ID \x00\x00\x00 EI\nQ\n".to_vec();
        }
        "shading_used" => {
            page_resources.set(
                "Shading",
                dictionary! {
                    "Sh1" => dictionary! {
                        "ShadingType" => 2,
                        "ColorSpace" => indirect_space,
                        "Coords" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                        "Function" => dictionary! {
                            "FunctionType" => 2,
                            "Domain" => vec![0.into(), 1.into()],
                            "C0" => vec![0.into(), 0.into(), 0.into()],
                            "C1" => vec![1.into(), 1.into(), 1.into()],
                            "N" => 1,
                        },
                        "Extend" => vec![Object::Boolean(true), Object::Boolean(true)],
                    },
                },
            );
            page_contents = b"/Sh1 sh\n".to_vec();
        }
        "indexed_base_used" => {
            page_resources.set(
                "ColorSpace",
                dictionary! {
                    "CS1" => Object::Array(vec![
                        Object::Name(b"Indexed".to_vec()),
                        indirect_space,
                        Object::Integer(1),
                        Object::String(vec![0; 6], lopdf::StringFormat::Hexadecimal),
                    ]),
                },
            );
        }
        "cyclic_indexed" => {
            let first_id = document.new_object_id();
            let second_id = document.new_object_id();
            document.objects.insert(
                first_id,
                Object::Array(vec![
                    Object::Name(b"Indexed".to_vec()),
                    Object::Reference(second_id),
                    Object::Integer(1),
                    Object::String(vec![0; 6], lopdf::StringFormat::Hexadecimal),
                ]),
            );
            document.objects.insert(
                second_id,
                Object::Array(vec![
                    Object::Name(b"Indexed".to_vec()),
                    Object::Reference(first_id),
                    Object::Integer(1),
                    Object::String(vec![0; 6], lopdf::StringFormat::Hexadecimal),
                ]),
            );
            page_resources.set(
                "ColorSpace",
                dictionary! {"CS1" => Object::Reference(first_id)},
            );
        }
        "deep_indexed" => {
            let mut nested = indirect_space;
            for _ in 0..8 {
                nested = Object::Array(vec![
                    Object::Name(b"Indexed".to_vec()),
                    nested,
                    Object::Integer(1),
                    Object::String(vec![0; 6], lopdf::StringFormat::Hexadecimal),
                ]);
            }
            page_resources.set("ColorSpace", dictionary! {"CS1" => nested});
        }
        _ => panic!("unknown ICCBased fixture case {case}"),
    }

    let content_id = document.add_object(Stream::new(Dictionary::new(), page_contents));
    let mut page = dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => content_id,
    };
    if case != "inherited_resources" {
        page.set("Resources", page_resources.clone());
    }
    let page_id = document.add_object(page);
    let mut pages = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![Object::Reference(page_id)],
        "Count" => 1,
    };
    if case == "inherited_resources" {
        pages.set("Resources", page_resources);
    }
    document.objects.insert(pages_id, Object::Dictionary(pages));
    let metadata_id = standard_metadata_stream(&mut document);
    let output_intents = single_profile_intent(&mut document, valid, Some("GTS_PDFA1"));
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "OutputIntents" => output_intents.expect("output intent"),
    });
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).expect("save ICCBased fixture");
    bytes
}

pub fn icc_cmyk_overprint_fixture(case: &str) -> Vec<u8> {
    let mut document = pdf_document();
    let pages_id = document.new_object_id();
    let profile = profile_reference(&mut document, icc_header(*b"mntr", *b"CMYK", 2, 1));
    let color_space = Object::Array(vec![Object::Name(b"ICCBased".to_vec()), profile]);
    let state = match case {
        "stroke_opm_one" | "fill_opm_one" | "select_before_state" => {
            dictionary! { "OP" => true, "op" => true, "OPM" => 1 }
        }
        "stroke_opm_zero" => dictionary! { "OP" => true, "OPM" => 0 },
        "stroke_overprint_false" => dictionary! { "OP" => false, "OPM" => 1 },
        _ => panic!("unknown ICCBased CMYK overprint fixture {case}"),
    };
    let contents = match case {
        "stroke_opm_one" | "stroke_opm_zero" | "stroke_overprint_false" => {
            b"/GS1 gs\n/CS1 CS\n0 0 0 0 SC\n0 0 m\n1 1 l\nS\n".to_vec()
        }
        "fill_opm_one" => b"/GS1 gs\n/CS1 cs\n0 0 0 0 sc\n0 0 m\n1 1 l\nf\n".to_vec(),
        "select_before_state" => b"/CS1 CS\n/GS1 gs\n0 0 0 0 SC\n0 0 m\n1 1 l\nS\n".to_vec(),
        _ => panic!("inconsistent ICCBased CMYK overprint fixture {case}"),
    };
    let contents = document.add_object(Stream::new(Dictionary::new(), contents));
    let page = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
        "Resources" => dictionary! {
            "ColorSpace" => dictionary! { "CS1" => color_space },
            "ExtGState" => dictionary! { "GS1" => state },
        },
        "Contents" => contents,
    });
    wrap_pages(&mut document, pages_id, page);
    let metadata = standard_metadata_stream(&mut document);
    let output_intents = single_profile_intent(
        &mut document,
        icc_header(*b"mntr", *b"CMYK", 2, 1),
        Some("GTS_PDFA1"),
    );
    let catalog = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata,
        "OutputIntents" => output_intents.expect("output intent"),
    });
    document.trailer.set("Root", catalog);
    let info = document.add_object(complete_info());
    document.trailer.set("Info", info);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save ICCBased CMYK overprint fixture");
    bytes
}

pub fn device_color_fixture(case: &str) -> Vec<u8> {
    let mut document = pdf_document();
    let pages_id = document.new_object_id();
    let rgb_profile = icc_header(*b"mntr", *b"RGB ", 2, 1);
    let cmyk_profile = icc_header(*b"mntr", *b"CMYK", 2, 1);
    let mut resources = Dictionary::new();
    let mut contents = Vec::new();

    match case {
        "baseline" => {}
        "rgb_operator"
        | "rgb_with_cmyk_output"
        | "rgb_without_output"
        | "rgb_wrong_s"
        | "rgb_wrong_arity_with_cmyk_output" => {
            contents = if case == "rgb_wrong_arity_with_cmyk_output" {
                b"0 0 rg\n".to_vec()
            } else {
                b"0 0 0 rg\n0 0 0 RG\n".to_vec()
            };
        }
        "cmyk_operator" | "cmyk_with_cmyk_output" | "cmyk_without_output" => {
            contents = b"0 0 0 0 k\n0 0 0 0 K\n".to_vec();
        }
        "gray_operator" | "gray_with_cmyk_output" | "gray_without_output" => {
            contents = b"0 g\n0 G\n".to_vec();
        }
        "explicit_rgb" => contents = b"/DeviceRGB cs\n/DeviceRGB CS\n".to_vec(),
        "resource_rgb" | "unused_resource_rgb" => {
            resources.set("ColorSpace", dictionary! {"CS1" => "DeviceRGB"});
            if case == "resource_rgb" {
                contents = b"/CS1 cs\n".to_vec();
            }
        }
        "default_rgb_override" => {
            let profile = profile_reference(&mut document, rgb_profile.clone());
            resources.set(
                "ColorSpace",
                dictionary! {
                    "DefaultRGB" => Object::Array(vec![
                        Object::Name(b"ICCBased".to_vec()),
                        profile,
                    ]),
                },
            );
            contents = b"0 0 0 rg\n0 0 0 RG\n".to_vec();
        }
        "form_rgb" => {
            let form_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                },
                b"0 0 0 rg\n".to_vec(),
            ));
            resources.set("XObject", dictionary! {"Fm" => form_id});
            contents = b"/Fm Do\n".to_vec();
        }
        "image_rgb" => {
            let image_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 8,
                    "ColorSpace" => "DeviceRGB",
                },
                vec![0, 0, 0],
            ));
            resources.set("XObject", dictionary! {"Im" => image_id});
            contents = b"/Im Do\n".to_vec();
        }
        "inline_rgb" => {
            contents = b"q\nBI /W 1 /H 1 /BPC 8 /CS /RGB ID \x00\x00\x00 EI\nQ\n".to_vec();
        }
        "shading_rgb" => {
            resources.set(
                "Shading",
                dictionary! {
                    "Sh1" => dictionary! {
                        "ShadingType" => 2,
                        "ColorSpace" => "DeviceRGB",
                        "Coords" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                        "Function" => dictionary! {
                            "FunctionType" => 2,
                            "Domain" => vec![0.into(), 1.into()],
                            "C0" => vec![0.into(), 0.into(), 0.into()],
                            "C1" => vec![1.into(), 1.into(), 1.into()],
                            "N" => 1,
                        },
                    },
                },
            );
            contents = b"/Sh1 sh\n".to_vec();
        }
        "indexed_rgb" => {
            resources.set(
                "ColorSpace",
                dictionary! {
                    "CS1" => Object::Array(vec![
                        Object::Name(b"Indexed".to_vec()),
                        Object::Name(b"DeviceRGB".to_vec()),
                        Object::Integer(1),
                        Object::String(vec![0; 6], lopdf::StringFormat::Hexadecimal),
                    ]),
                },
            );
            contents = b"/CS1 cs\n".to_vec();
        }
        "separation_rgb"
        | "devicen_rgb"
        | "devicen_nine_components"
        | "devicen_33_components"
        | "separation_invalid_utf8"
        | "devicen_invalid_utf8"
        | "separation_unreferenced_invalid_utf8"
        | "devicen_unreferenced_invalid_utf8" => {
            let tint_transform = dictionary! {
                "FunctionType" => 2,
                "Domain" => vec![0.into(), 1.into()],
                "C0" => vec![0.into(), 0.into(), 0.into()],
                "C1" => vec![1.into(), 1.into(), 1.into()],
                "N" => 1,
            };
            let color_space = if matches!(
                case,
                "separation_rgb"
                    | "separation_invalid_utf8"
                    | "separation_unreferenced_invalid_utf8"
            ) {
                Object::Array(vec![
                    Object::Name(b"Separation".to_vec()),
                    Object::Name(
                        if matches!(
                            case,
                            "separation_invalid_utf8" | "separation_unreferenced_invalid_utf8"
                        ) {
                            b"Spot\xff".to_vec()
                        } else {
                            b"Spot".to_vec()
                        },
                    ),
                    Object::Name(b"DeviceRGB".to_vec()),
                    Object::Dictionary(tint_transform),
                ])
            } else {
                Object::Array(vec![
                    Object::Name(b"DeviceN".to_vec()),
                    Object::Array(
                        (0..match case {
                            "devicen_nine_components" => 9,
                            "devicen_33_components" => 33,
                            _ => 1,
                        })
                            .map(|index| {
                                Object::Name(
                                    if matches!(
                                        case,
                                        "devicen_invalid_utf8"
                                            | "devicen_unreferenced_invalid_utf8"
                                    ) && index == 0
                                    {
                                        b"Spot\xff".to_vec()
                                    } else {
                                        format!("Spot{index}").into_bytes()
                                    },
                                )
                            })
                            .collect(),
                    ),
                    Object::Name(b"DeviceRGB".to_vec()),
                    Object::Dictionary(tint_transform),
                ])
            };
            if matches!(
                case,
                "separation_unreferenced_invalid_utf8" | "devicen_unreferenced_invalid_utf8"
            ) {
                document.add_object(color_space);
            } else {
                resources.set("ColorSpace", dictionary! {"CS1" => color_space});
                contents = b"/CS1 cs\n".to_vec();
            }
        }
        "pattern_rgb" => {
            let pattern_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "Pattern",
                    "PatternType" => 1,
                    "PaintType" => 2,
                    "TilingType" => 1,
                    "BBox" => vec![0.into(), 0.into(), 1.into(), 1.into()],
                    "XStep" => 1,
                    "YStep" => 1,
                    "Resources" => Dictionary::new(),
                },
                b"0 0 1 1 re f\n".to_vec(),
            ));
            resources.set(
                "ColorSpace",
                dictionary! {
                    "CS1" => Object::Array(vec![
                        Object::Name(b"Pattern".to_vec()),
                        Object::Name(b"DeviceRGB".to_vec()),
                    ]),
                },
            );
            resources.set("Pattern", dictionary! {"P1" => pattern_id});
            contents = b"/CS1 cs\n1 0 0 /P1 scn\n0 0 10 10 re f\n".to_vec();
        }
        _ => panic!("unknown device-colour fixture case {case}"),
    }

    let contents_id = document.add_object(Stream::new(Dictionary::new(), contents));
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => resources,
        "Contents" => contents_id,
    });
    wrap_pages(&mut document, pages_id, page_id);

    let metadata_id = standard_metadata_stream(&mut document);
    let mut catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
    };
    if !matches!(
        case,
        "rgb_without_output" | "cmyk_without_output" | "gray_without_output"
    ) {
        let output_bytes = if matches!(
            case,
            "rgb_with_cmyk_output"
                | "rgb_wrong_arity_with_cmyk_output"
                | "cmyk_with_cmyk_output"
                | "gray_with_cmyk_output"
                | "unused_resource_rgb"
                | "default_rgb_override"
                | "explicit_rgb"
                | "resource_rgb"
                | "form_rgb"
                | "image_rgb"
                | "inline_rgb"
                | "shading_rgb"
                | "indexed_rgb"
                | "separation_rgb"
                | "devicen_rgb"
                | "devicen_nine_components"
                | "devicen_33_components"
                | "pattern_rgb"
        ) {
            cmyk_profile
        } else {
            rgb_profile
        };
        let subtype = if case == "rgb_wrong_s" {
            "GTS_PDFX"
        } else {
            "GTS_PDFA1"
        };
        let output_intents = single_profile_intent(&mut document, output_bytes, Some(subtype));
        catalog.set("OutputIntents", output_intents.expect("output intent"));
    }
    let catalog_id = document.add_object(catalog);
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save device-colour fixture");
    bytes
}

/// Focused fixtures for colour-source applicability probes that are shared by
/// ICCBased, device-colour, and rendering-intent validation. These deliberately
/// keep the source shape atomic so the pinned veraPDF rule-ID delta identifies
/// whether that source enters the corresponding model population.
pub fn color_path_fixture(case: &str) -> Vec<u8> {
    let mut document = pdf_document();
    let pages_id = document.new_object_id();
    let rgb_profile = icc_header(*b"mntr", *b"RGB ", 2, 1);
    let cmyk_profile = icc_header(*b"mntr", *b"CMYK", 2, 1);
    let invalid_profile = profile_reference(&mut document, icc_header(*b"link", *b"RGB ", 2, 1));
    let invalid_icc = Object::Array(vec![Object::Name(b"ICCBased".to_vec()), invalid_profile]);
    let valid_icc_profile = profile_reference(&mut document, rgb_profile.clone());
    let valid_icc = Object::Array(vec![Object::Name(b"ICCBased".to_vec()), valid_icc_profile]);
    let valid_default_profile = profile_reference(&mut document, rgb_profile.clone());
    let valid_default_rgb = Object::Array(vec![
        Object::Name(b"ICCBased".to_vec()),
        valid_default_profile,
    ]);
    let valid_default_cmyk_profile = profile_reference(&mut document, cmyk_profile.clone());
    let valid_default_cmyk = Object::Array(vec![
        Object::Name(b"ICCBased".to_vec()),
        valid_default_cmyk_profile,
    ]);
    let valid_default_gray_profile =
        profile_reference(&mut document, icc_header(*b"mntr", *b"GRAY", 2, 1));
    let valid_default_gray = Object::Array(vec![
        Object::Name(b"ICCBased".to_vec()),
        valid_default_gray_profile,
    ]);

    let mut resources = Dictionary::new();
    let mut contents = Vec::new();
    let mut group = None;
    let mut annotations = Vec::new();
    let mut output_intents_override = None;
    let mut inherit_page_resources = false;
    let mut omit_output_intents = false;

    let tint_transform = || {
        Object::Dictionary(dictionary! {
            "FunctionType" => 2,
            "Domain" => vec![0.into(), 1.into()],
            "C0" => vec![0.into(), 0.into(), 0.into()],
            "C1" => vec![1.into(), 1.into(), 1.into()],
            "N" => 1,
        })
    };
    let separation = |alternate: Object| {
        Object::Array(vec![
            Object::Name(b"Separation".to_vec()),
            Object::Name(b"Spot".to_vec()),
            alternate,
            tint_transform(),
        ])
    };
    let devicen = |alternate: Object| {
        Object::Array(vec![
            Object::Name(b"DeviceN".to_vec()),
            Object::Array(vec![Object::Name(b"Spot".to_vec())]),
            alternate,
            tint_transform(),
        ])
    };

    match case {
        "icc_baseline" | "device_baseline" | "intent_baseline" => {}
        "separation_consistent"
        | "separation_inconsistent"
        | "separation_unreferenced_inconsistent" => {
            let first = separation(Object::Name(b"DeviceRGB".to_vec()));
            let second = if case == "separation_inconsistent" {
                separation(Object::Name(b"DeviceCMYK".to_vec()))
            } else {
                first.clone()
            };
            if case == "separation_unreferenced_inconsistent" {
                document.add_object(first);
                document.add_object(second);
            } else {
                resources.set(
                    "ColorSpace",
                    dictionary! { "CS1" => first, "CS2" => second },
                );
                contents = b"/CS1 cs\n/CS2 cs\n".to_vec();
            }
        }
        "icc_separation_alternate" | "icc_separation_valid" => {
            let alternate = if case == "icc_separation_valid" {
                valid_icc.clone()
            } else {
                invalid_icc.clone()
            };
            resources.set("ColorSpace", dictionary! {"CS1" => separation(alternate)});
            contents = b"/CS1 cs\n".to_vec();
        }
        "icc_devicen_alternate" | "icc_devicen_valid" | "icc_devicen_wrong_n" => {
            let alternate = if case == "icc_devicen_valid" {
                valid_icc.clone()
            } else if case == "icc_devicen_wrong_n" {
                let profile =
                    document.add_object(Stream::new(dictionary! {"N" => 4}, rgb_profile.clone()));
                Object::Array(vec![
                    Object::Name(b"ICCBased".to_vec()),
                    Object::Reference(profile),
                ])
            } else {
                invalid_icc.clone()
            };
            resources.set("ColorSpace", dictionary! {"CS1" => devicen(alternate)});
            contents = b"/CS1 cs\n".to_vec();
        }
        "icc_page_group"
        | "icc_page_group_valid"
        | "icc_page_group_wrong_type"
        | "icc_page_group_inherited" => {
            let color_space = match case {
                "icc_page_group_valid" => valid_icc.clone(),
                "icc_page_group_wrong_type" => Object::Integer(7),
                "icc_page_group_inherited" => {
                    resources.set("ColorSpace", dictionary! {"CS1" => invalid_icc.clone()});
                    inherit_page_resources = true;
                    Object::Name(b"CS1".to_vec())
                }
                _ => invalid_icc.clone(),
            };
            group = Some(dictionary! {
                "Type" => "Group",
                "S" => "Transparency",
                "CS" => color_space,
            });
        }
        "icc_form_group" => {
            let form = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Group" => dictionary! {
                        "Type" => "Group",
                        "S" => "Transparency",
                        "CS" => invalid_icc.clone(),
                    },
                },
                Vec::new(),
            ));
            resources.set("XObject", dictionary! {"Fm" => form});
            contents = b"/Fm Do\n".to_vec();
        }
        "icc_soft_mask_group" | "device_soft_mask_group" | "intent_soft_mask_group" => {
            let (group_resources, group_contents) = match case {
                "icc_soft_mask_group" => (
                    dictionary! {"ColorSpace" => dictionary! {"CS1" => invalid_icc.clone()}},
                    b"/CS1 cs\n".to_vec(),
                ),
                "device_soft_mask_group" => (Dictionary::new(), b"0 0 0 rg\n".to_vec()),
                _ => (Dictionary::new(), b"/MaiIntent ri\n".to_vec()),
            };
            let form = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Resources" => group_resources,
                    "Group" => dictionary! {
                        "Type" => "Group",
                        "S" => "Transparency",
                        "CS" => "DeviceGray",
                    },
                },
                group_contents,
            ));
            let state = document.add_object(dictionary! {
                "Type" => "ExtGState",
                "SMask" => dictionary! {"S" => "Alpha", "G" => form},
            });
            resources.set("ExtGState", dictionary! {"GS1" => state});
            contents = b"/GS1 gs\n0 0 10 10 re f\n".to_vec();
        }
        "icc_annotation_appearance"
        | "icc_annotation_appearance_valid"
        | "icc_annotation_state"
        | "icc_annotation_unreferenced"
        | "device_annotation_appearance"
        | "device_annotation_unreferenced"
        | "intent_annotation_appearance"
        | "intent_annotation_valid"
        | "intent_annotation_state"
        | "intent_annotation_down"
        | "intent_annotation_unreferenced" => {
            let (appearance_resources, appearance_contents) = match case {
                "icc_annotation_appearance" => (
                    dictionary! {"ColorSpace" => dictionary! {"CS1" => invalid_icc.clone()}},
                    b"/CS1 cs\n".to_vec(),
                ),
                "icc_annotation_appearance_valid" => (
                    dictionary! {"ColorSpace" => dictionary! {"CS1" => valid_icc.clone()}},
                    b"/CS1 cs\n".to_vec(),
                ),
                "icc_annotation_state" => (
                    dictionary! {"ColorSpace" => dictionary! {"CS1" => invalid_icc.clone()}},
                    b"/CS1 cs\n".to_vec(),
                ),
                "icc_annotation_unreferenced" => (
                    dictionary! {"ColorSpace" => dictionary! {"CS1" => invalid_icc.clone()}},
                    b"/CS1 cs\n".to_vec(),
                ),
                "device_annotation_appearance" | "device_annotation_unreferenced" => {
                    (Dictionary::new(), b"0 0 0 rg\n".to_vec())
                }
                "intent_annotation_valid" => (Dictionary::new(), b"/Perceptual ri\n".to_vec()),
                _ => (Dictionary::new(), b"/MaiIntent ri\n".to_vec()),
            };
            let appearance = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Resources" => appearance_resources,
                },
                appearance_contents,
            ));
            let mut annotation = dictionary! {
                "Type" => "Annot",
                "Subtype" => "Text",
                "Rect" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                "F" => 4,
                "AP" => dictionary! {"N" => appearance},
            };
            if matches!(case, "icc_annotation_state" | "intent_annotation_state") {
                let selected = document.add_object(Stream::new(
                    dictionary! {
                        "Type" => "XObject",
                        "Subtype" => "Form",
                        "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    },
                    Vec::new(),
                ));
                annotation.set("Subtype", "Widget");
                annotation.set("FT", "Btn");
                annotation.set("AS", "On");
                annotation.set(
                    "AP",
                    dictionary! {
                        "N" => dictionary! {
                            "On" => selected,
                            "Off" => appearance,
                        },
                    },
                );
            } else if case == "intent_annotation_down" {
                let normal = document.add_object(Stream::new(
                    dictionary! {
                        "Type" => "XObject",
                        "Subtype" => "Form",
                        "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    },
                    Vec::new(),
                ));
                annotation.set(
                    "AP",
                    dictionary! {
                        "N" => normal,
                        "D" => appearance,
                    },
                );
            }
            let annotation = Object::Reference(document.add_object(annotation));
            if !case.ends_with("_unreferenced") {
                annotations.push(annotation);
            }
        }
        "icc_pattern_content"
        | "icc_pattern_content_valid"
        | "icc_pattern_unused"
        | "icc_pattern_without_selection"
        | "device_pattern_content"
        | "device_pattern_unused"
        | "intent_pattern_content"
        | "intent_pattern_valid"
        | "intent_pattern_unused" => {
            let (pattern_resources, pattern_contents) = match case {
                "icc_pattern_content" => (
                    dictionary! {"ColorSpace" => dictionary! {"CS1" => invalid_icc.clone()}},
                    b"/CS1 cs\n0 0 1 1 re f\n".to_vec(),
                ),
                "icc_pattern_content_valid" => (
                    dictionary! {"ColorSpace" => dictionary! {"CS1" => valid_icc.clone()}},
                    b"/CS1 cs\n0 0 1 1 re f\n".to_vec(),
                ),
                "icc_pattern_unused" => (
                    dictionary! {"ColorSpace" => dictionary! {"CS1" => invalid_icc.clone()}},
                    b"/CS1 cs\n0 0 1 1 re f\n".to_vec(),
                ),
                "icc_pattern_without_selection" => (
                    dictionary! {"ColorSpace" => dictionary! {"CS1" => invalid_icc.clone()}},
                    b"/CS1 cs\n0 0 1 1 re f\n".to_vec(),
                ),
                "device_pattern_content" | "device_pattern_unused" => {
                    (Dictionary::new(), b"0 0 0 rg\n0 0 1 1 re f\n".to_vec())
                }
                "intent_pattern_valid" => (
                    Dictionary::new(),
                    b"/Perceptual ri\n0 0 1 1 re f\n".to_vec(),
                ),
                _ => (Dictionary::new(), b"/MaiIntent ri\n0 0 1 1 re f\n".to_vec()),
            };
            let pattern = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "Pattern",
                    "PatternType" => 1,
                    "PaintType" => 1,
                    "TilingType" => 1,
                    "BBox" => vec![0.into(), 0.into(), 1.into(), 1.into()],
                    "XStep" => 1,
                    "YStep" => 1,
                    "Resources" => pattern_resources,
                },
                pattern_contents,
            ));
            resources.set("Pattern", dictionary! {"P1" => pattern});
            if case == "icc_pattern_without_selection" {
                contents = b"/P1 scn\n0 0 10 10 re f\n".to_vec();
            } else if !matches!(
                case,
                "icc_pattern_unused" | "device_pattern_unused" | "intent_pattern_unused"
            ) {
                contents = b"/Pattern cs\n/P1 scn\n0 0 10 10 re f\n".to_vec();
            }
        }
        "icc_pattern_underlying" => {
            let pattern = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "Pattern",
                    "PatternType" => 1,
                    "PaintType" => 2,
                    "TilingType" => 1,
                    "BBox" => vec![0.into(), 0.into(), 1.into(), 1.into()],
                    "XStep" => 1,
                    "YStep" => 1,
                    "Resources" => Dictionary::new(),
                },
                b"0 0 1 1 re f\n".to_vec(),
            ));
            resources.set(
                "ColorSpace",
                dictionary! {
                    "CS1" => Object::Array(vec![
                        Object::Name(b"Pattern".to_vec()),
                        invalid_icc.clone(),
                    ]),
                },
            );
            resources.set("Pattern", dictionary! {"P1" => pattern});
            contents = b"/CS1 cs\n0 0 0 /P1 scn\n0 0 10 10 re f\n".to_vec();
        }
        "icc_shading_pattern_direct"
        | "icc_shading_pattern_indirect"
        | "icc_shading_pattern_unused" => {
            let shading = dictionary! {
                "ShadingType" => 2,
                "ColorSpace" => invalid_icc.clone(),
                "Coords" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                "Function" => dictionary! {
                    "FunctionType" => 2,
                    "Domain" => vec![0.into(), 1.into()],
                    "C0" => vec![0.into(), 0.into(), 0.into()],
                    "C1" => vec![1.into(), 1.into(), 1.into()],
                    "N" => 1,
                },
                "Extend" => vec![Object::Boolean(true), Object::Boolean(true)],
            };
            let pattern = dictionary! {
                "Type" => "Pattern",
                "PatternType" => 2,
                "Shading" => if case == "icc_shading_pattern_indirect" {
                    Object::Reference(document.add_object(shading))
                } else {
                    Object::Dictionary(shading)
                },
            };
            let pattern = if case == "icc_shading_pattern_direct" {
                Object::Dictionary(pattern)
            } else {
                Object::Reference(document.add_object(pattern))
            };
            resources.set("Pattern", dictionary! {"P1" => pattern});
            if case != "icc_shading_pattern_unused" {
                contents = b"/Pattern cs\n/P1 scn\n".to_vec();
            }
        }
        "device_page_group" => {
            group = Some(dictionary! {
                "Type" => "Group",
                "S" => "Transparency",
                "CS" => "DeviceRGB",
            });
        }
        "device_form_group" => {
            let form = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Group" => dictionary! {
                        "Type" => "Group",
                        "S" => "Transparency",
                        "CS" => "DeviceRGB",
                    },
                },
                Vec::new(),
            ));
            resources.set("XObject", dictionary! {"Fm" => form});
            contents = b"/Fm Do\n".to_vec();
        }
        "device_image_default"
        | "device_image_inherited_default"
        | "device_image_default_wrong_type"
        | "device_inline_default"
        | "device_indexed_default"
        | "device_separation_default"
        | "device_devicen_default" => {
            let default_rgb = if case == "device_image_default_wrong_type" {
                Object::Integer(7)
            } else {
                valid_default_rgb.clone()
            };
            resources.set("ColorSpace", dictionary! {"DefaultRGB" => default_rgb});
            match case {
                "device_image_default"
                | "device_image_inherited_default"
                | "device_image_default_wrong_type" => {
                    inherit_page_resources = case == "device_image_inherited_default";
                    let image = document.add_object(Stream::new(
                        dictionary! {
                            "Type" => "XObject",
                            "Subtype" => "Image",
                            "Width" => 1,
                            "Height" => 1,
                            "BitsPerComponent" => 8,
                            "ColorSpace" => "DeviceRGB",
                        },
                        vec![0, 0, 0],
                    ));
                    resources.set("XObject", dictionary! {"Im" => image});
                    contents = b"/Im Do\n".to_vec();
                }
                "device_inline_default" => {
                    contents = b"BI /W 1 /H 1 /BPC 8 /CS /RGB ID \x00\x00\x00 EI\n".to_vec();
                }
                "device_indexed_default" => {
                    resources
                        .get_mut(b"ColorSpace")
                        .expect("ColorSpace")
                        .as_dict_mut()
                        .expect("dictionary")
                        .set(
                            "CS1",
                            Object::Array(vec![
                                Object::Name(b"Indexed".to_vec()),
                                Object::Name(b"DeviceRGB".to_vec()),
                                Object::Integer(1),
                                Object::String(vec![0; 6], StringFormat::Hexadecimal),
                            ]),
                        );
                    contents = b"/CS1 cs\n".to_vec();
                }
                "device_separation_default" => {
                    resources
                        .get_mut(b"ColorSpace")
                        .expect("ColorSpace")
                        .as_dict_mut()
                        .expect("dictionary")
                        .set("CS1", separation(Object::Name(b"DeviceRGB".to_vec())));
                    contents = b"/CS1 cs\n".to_vec();
                }
                "device_devicen_default" => {
                    resources
                        .get_mut(b"ColorSpace")
                        .expect("ColorSpace")
                        .as_dict_mut()
                        .expect("dictionary")
                        .set("CS1", devicen(Object::Name(b"DeviceRGB".to_vec())));
                    contents = b"/CS1 cs\n".to_vec();
                }
                _ => panic!("inconsistent device-color fixture {case}"),
            }
        }
        "device_output_invalid_rgb" => {
            contents = b"0 0 0 rg\n".to_vec();
            output_intents_override = single_profile_intent(
                &mut document,
                icc_header(*b"link", *b"RGB ", 2, 1),
                Some("GTS_PDFA1"),
            );
        }
        "device_output_truncated" => {
            contents = b"0 0 0 rg\n".to_vec();
            output_intents_override =
                single_profile_intent(&mut document, vec![0; 19], Some("GTS_PDFA1"));
        }
        "device_output_rgb_then_cmyk" | "device_output_cmyk_then_rgb" => {
            contents = b"0 0 0 rg\n".to_vec();
            let rgb = profile_reference(&mut document, rgb_profile.clone());
            let cmyk = profile_reference(&mut document, cmyk_profile.clone());
            output_intents_override = if case == "device_output_rgb_then_cmyk" {
                two_intents(&mut document, rgb, cmyk)
            } else {
                two_intents(&mut document, cmyk, rgb)
            };
        }
        "device_output_pdfa_rgb_then_wrong_s_cmyk" => {
            contents = b"0 0 0 rg\n".to_vec();
            let rgb = profile_reference(&mut document, rgb_profile.clone());
            let cmyk = profile_reference(&mut document, cmyk_profile.clone());
            let first = document.add_object(output_intent_dictionary(Some(rgb), Some("GTS_PDFA1")));
            let second =
                document.add_object(output_intent_dictionary(Some(cmyk), Some("GTS_PDFX")));
            output_intents_override = Some(Object::Array(vec![
                Object::Reference(first),
                Object::Reference(second),
            ]));
        }
        "device_cmyk_image_rgb_output"
        | "device_cmyk_image_default"
        | "device_gray_image_no_output"
        | "device_gray_image_default" => {
            let (device_space, default_name, default_space) = match case {
                "device_cmyk_image_rgb_output" => ("DeviceCMYK", None, None),
                "device_cmyk_image_default" => (
                    "DeviceCMYK",
                    Some("DefaultCMYK"),
                    Some(valid_default_cmyk.clone()),
                ),
                "device_gray_image_no_output" => ("DeviceGray", None, None),
                _ => (
                    "DeviceGray",
                    Some("DefaultGray"),
                    Some(valid_default_gray.clone()),
                ),
            };
            if let (Some(default_name), Some(default_space)) = (default_name, default_space) {
                resources.set(
                    "ColorSpace",
                    Dictionary::from_iter([(default_name.as_bytes(), default_space)]),
                );
            }
            let image = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 8,
                    "ColorSpace" => Object::Name(device_space.as_bytes().to_vec()),
                },
                if device_space == "DeviceCMYK" {
                    vec![0, 0, 0, 0]
                } else {
                    vec![0]
                },
            ));
            resources.set("XObject", dictionary! {"Im" => image});
            contents = b"/Im Do\n".to_vec();
            if device_space == "DeviceCMYK" {
                output_intents_override =
                    single_profile_intent(&mut document, rgb_profile.clone(), Some("GTS_PDFA1"));
            } else {
                omit_output_intents = true;
            }
        }
        "intent_inline_image" | "intent_inline_valid" | "intent_inline_wrong_type" => {
            contents = match case {
                "intent_inline_valid" => {
                    b"BI /W 1 /H 1 /BPC 8 /CS /RGB /Intent /Perceptual ID \x00\x00\x00 EI\n"
                        .to_vec()
                }
                "intent_inline_wrong_type" => {
                    b"BI /W 1 /H 1 /BPC 8 /CS /RGB /Intent 7 ID \x00\x00\x00 EI\n".to_vec()
                }
                _ => {
                    b"BI /W 1 /H 1 /BPC 8 /CS /RGB /Intent /MaiIntent ID \x00\x00\x00 EI\n".to_vec()
                }
            };
        }
        "intent_alternate_image" => {
            let alternate = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 8,
                    "ColorSpace" => "DeviceRGB",
                    "Intent" => "MaiIntent",
                },
                vec![0, 0, 0],
            ));
            let primary = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 8,
                    "ColorSpace" => "DeviceRGB",
                    "Alternates" => vec![Object::Dictionary(dictionary! {
                        "Image" => alternate,
                        "DefaultForPrinting" => true,
                    })],
                },
                vec![0, 0, 0],
            ));
            resources.set("XObject", dictionary! {"Im" => primary});
            contents = b"/Im Do\n".to_vec();
        }
        _ => panic!("unknown colour-path fixture case {case}"),
    }

    let contents_id = document.add_object(Stream::new(Dictionary::new(), contents));
    let mut page = dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => contents_id,
    };
    if !inherit_page_resources {
        page.set("Resources", resources.clone());
    }
    if let Some(group) = group {
        page.set("Group", group);
    }
    if !annotations.is_empty() {
        page.set("Annots", Object::Array(annotations));
    }
    let page_id = document.add_object(page);
    if inherit_page_resources {
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => 1,
                "Resources" => resources,
            }),
        );
    } else {
        wrap_pages(&mut document, pages_id, page_id);
    }

    let metadata_id = standard_metadata_stream(&mut document);
    let output_profile = if case.starts_with("device_") {
        cmyk_profile
    } else {
        rgb_profile
    };
    let mut catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
    };
    if !omit_output_intents {
        let output_intents = output_intents_override
            .or_else(|| single_profile_intent(&mut document, output_profile, Some("GTS_PDFA1")));
        catalog.set("OutputIntents", output_intents.expect("output intent"));
    }
    let catalog_id = document.add_object(catalog);
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save colour-path fixture");
    bytes
}

pub fn xobject_fixture(case: &str) -> Vec<u8> {
    let mut document = pdf_document();
    let pages_id = document.new_object_id();
    let mut resources = Dictionary::new();
    let mut contents = b"/XO Do\n".to_vec();
    let mut include_resource = true;
    let inherit_resources = case == "inherited_xobject_image_bpc_16";
    let explicit_mask_id = (case == "explicit_mask_bpc_8").then(|| {
        document.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 1,
                "Height" => 1,
                "BitsPerComponent" => 8,
            },
            vec![0],
        ))
    });

    let xobject = match case {
        "baseline" | "image_interpolate_false" | "image_bpc_missing" => {
            let mut dictionary = dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 1,
                "Height" => 1,
                "ColorSpace" => "DeviceRGB",
            };
            if case == "baseline" {
                dictionary.set("BitsPerComponent", 8);
            } else if case == "image_interpolate_false" {
                dictionary.set("BitsPerComponent", 8);
                dictionary.set("Interpolate", false);
            }
            Stream::new(dictionary, vec![0, 0, 0])
        }
        "image_alternates"
        | "image_alternates_null"
        | "image_opi"
        | "image_opi_null"
        | "image_interpolate_true"
        | "image_interpolate_indirect_true"
        | "image_interpolate_null"
        | "image_bpc_16"
        | "image_bpc_3"
        | "image_bpc_indirect_16"
        | "image_subtype_indirect_bpc_16"
        | "image_mask_indirect_true_bpc_16"
        | "direct_image_bpc_16"
        | "indirect_xobject_dictionary_image_bpc_16"
        | "inherited_xobject_image_bpc_16"
        | "shared_painted_explicit_mask_bpc_16"
        | "unused_resource_invalid_image"
        | "unreferenced_invalid_image"
        | "two_invalid_images" => {
            let mut dictionary = dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 1,
                "Height" => 1,
                "BitsPerComponent" => 8,
                "ColorSpace" => "DeviceRGB",
            };
            match case {
                "image_alternates" => dictionary.set("Alternates", Vec::<Object>::new()),
                "image_alternates_null" => dictionary.set("Alternates", Object::Null),
                "image_opi" => dictionary.set("OPI", Dictionary::new()),
                "image_opi_null" => dictionary.set("OPI", Object::Null),
                "image_interpolate_true" => dictionary.set("Interpolate", true),
                "image_interpolate_indirect_true" => {
                    dictionary.set("Interpolate", document.add_object(Object::Boolean(true)))
                }
                "image_interpolate_null" => dictionary.set("Interpolate", Object::Null),
                "image_bpc_16"
                | "image_bpc_3"
                | "image_bpc_indirect_16"
                | "image_subtype_indirect_bpc_16"
                | "image_mask_indirect_true_bpc_16"
                | "direct_image_bpc_16"
                | "indirect_xobject_dictionary_image_bpc_16"
                | "inherited_xobject_image_bpc_16"
                | "shared_painted_explicit_mask_bpc_16"
                | "unused_resource_invalid_image"
                | "unreferenced_invalid_image"
                | "two_invalid_images" => dictionary.set(
                    "BitsPerComponent",
                    if case == "image_bpc_3" { 3 } else { 16 },
                ),
                _ => panic!("inconsistent image fixture {case}"),
            }
            if case == "image_bpc_indirect_16" {
                dictionary.set("BitsPerComponent", document.add_object(Object::Integer(16)));
            }
            if case == "image_subtype_indirect_bpc_16" {
                dictionary.set(
                    "Subtype",
                    document.add_object(Object::Name(b"Image".to_vec())),
                );
            }
            if case == "image_mask_indirect_true_bpc_16" {
                dictionary.set("ImageMask", document.add_object(Object::Boolean(true)));
            }
            if case == "unused_resource_invalid_image" {
                contents.clear();
            } else if case == "unreferenced_invalid_image" {
                contents.clear();
                include_resource = false;
            }
            Stream::new(dictionary, vec![0, 0, 0])
        }
        "mask_bpc_8" | "mask_bpc_missing" => {
            let mut dictionary = dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 1,
                "Height" => 1,
                "ImageMask" => true,
            };
            if case == "mask_bpc_8" {
                dictionary.set("BitsPerComponent", 8);
            }
            Stream::new(dictionary, vec![0])
        }
        "explicit_mask_bpc_8" => Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 1,
                "Height" => 1,
                "BitsPerComponent" => 8,
                "ColorSpace" => "DeviceRGB",
                "Mask" => explicit_mask_id.expect("explicit mask object"),
            },
            vec![0, 0, 0],
        ),
        "form_opi"
        | "form_opi_null"
        | "form_ps_key"
        | "form_ps_null"
        | "form_subtype2_ps"
        | "form_subtype2_indirect_ps"
        | "form_ref"
        | "direct_form_ref"
        | "form_ref_null" => {
            let mut dictionary = dictionary! {
                "Type" => "XObject",
                "Subtype" => "Form",
                "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
            };
            match case {
                "form_opi" => dictionary.set("OPI", Dictionary::new()),
                "form_opi_null" => dictionary.set("OPI", Object::Null),
                "form_ps_key" => {
                    let postscript =
                        document.add_object(Stream::new(Dictionary::new(), b"%!PS\n".to_vec()));
                    dictionary.set("PS", postscript);
                }
                "form_ps_null" => dictionary.set("PS", Object::Null),
                "form_subtype2_ps" => dictionary.set("Subtype2", "PS"),
                "form_subtype2_indirect_ps" => dictionary.set(
                    "Subtype2",
                    document.add_object(Object::Name(b"PS".to_vec())),
                ),
                "form_ref" | "direct_form_ref" => dictionary.set("Ref", Dictionary::new()),
                "form_ref_null" => dictionary.set("Ref", Object::Null),
                _ => panic!("inconsistent form XObject fixture {case}"),
            }
            Stream::new(dictionary, Vec::new())
        }
        "postscript_xobject" | "direct_postscript_xobject" | "postscript_subtype_indirect" => {
            let mut dictionary = dictionary! {
                "Type" => "XObject",
                "Subtype" => "PS",
            };
            if case == "postscript_subtype_indirect" {
                dictionary.set("Subtype", document.add_object(Object::Name(b"PS".to_vec())));
            }
            Stream::new(dictionary, b"%!PS\n".to_vec())
        }
        _ => panic!("unknown XObject fixture case {case}"),
    };
    let xobject = if matches!(
        case,
        "direct_image_bpc_16" | "direct_form_ref" | "direct_postscript_xobject"
    ) {
        Object::Dictionary(xobject.dict)
    } else {
        Object::Reference(document.add_object(xobject))
    };
    if include_resource {
        let mut xobjects = dictionary! {"XO" => xobject.clone()};
        if case == "two_invalid_images" {
            let second_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 16,
                    "ColorSpace" => "DeviceRGB",
                },
                vec![0, 0, 0],
            ));
            xobjects.set("XO2", second_id);
            contents = b"/XO Do\n/XO2 Do\n".to_vec();
        }
        if case == "shared_painted_explicit_mask_bpc_16" {
            let primary = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 8,
                    "ColorSpace" => "DeviceRGB",
                    "Mask" => xobject.clone(),
                },
                vec![0, 0, 0],
            ));
            xobjects.set("Primary", primary);
            contents = b"/XO Do\n/Primary Do\n".to_vec();
        }
        if case == "indirect_xobject_dictionary_image_bpc_16" {
            resources.set("XObject", document.add_object(xobjects));
        } else {
            resources.set("XObject", xobjects);
        }
    }

    let contents_id = document.add_object(Stream::new(Dictionary::new(), contents));
    let mut page = dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => contents_id,
    };
    if !inherit_resources {
        page.set("Resources", resources.clone());
    }
    let page_id = document.add_object(page);
    if inherit_resources {
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => 1,
                "Resources" => resources,
            }),
        );
    } else {
        wrap_pages(&mut document, pages_id, page_id);
    }
    let metadata_id = standard_metadata_stream(&mut document);
    let output_intents = single_profile_intent(
        &mut document,
        icc_header(*b"mntr", *b"RGB ", 2, 1),
        Some("GTS_PDFA1"),
    );
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "OutputIntents" => output_intents.expect("output intent"),
    });
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).expect("save XObject fixture");
    bytes
}

pub fn graphics_fixture(case: &str) -> Vec<u8> {
    if case.starts_with("path_") {
        return graphics_content_path_fixture(case);
    }
    let mut document = pdf_document();
    let pages_id = document.new_object_id();
    let mut resources = Dictionary::new();
    let mut contents = Vec::new();
    let mut page_group = None;
    let mut annotations = Vec::new();
    let mut inherit_resources = false;
    let mut omit_output_intent = false;

    match case {
        "baseline" => {}
        "extgstate_transparency_no_output_intent" => {
            omit_output_intent = true;
            let state = document.add_object(dictionary! { "ca" => 0.5, "BM" => "Multiply" });
            resources.set("ExtGState", dictionary! { "GS1" => state });
            contents = b"/GS1 gs\n0 0 10 10 re f\n".to_vec();
            page_group = Some(dictionary! {
                "Type" => "Group",
                "S" => "Transparency",
            });
        }
        "inherited_resource_color_space"
        | "inherited_resource_calgray"
        | "inherited_resource_default_color_space"
        | "inherited_resource_extgstate"
        | "inherited_resource_font"
        | "inherited_resource_xobject"
        | "inherited_resource_pattern"
        | "inherited_resource_shading"
        | "inherited_resource_properties" => {
            inherit_resources = true;
            match case {
                "inherited_resource_color_space" => {
                    resources.set("ColorSpace", dictionary! { "CS1" => "DeviceRGB" });
                    contents = b"/CS1 cs\n".to_vec();
                }
                "inherited_resource_calgray" => {
                    resources.set(
                        "ColorSpace",
                        dictionary! { "CS1" => vec![
                            Object::Name(b"CalGray".to_vec()),
                            Object::Dictionary(dictionary! { "WhitePoint" => vec![1.into(), 1.into(), 1.into()] }),
                        ] },
                    );
                    contents = b"/CS1 cs\n".to_vec();
                }
                "inherited_resource_default_color_space" => {
                    resources.set("ColorSpace", dictionary! { "DefaultRGB" => "DeviceCMYK" });
                    contents = b"1 0 0 rg\n".to_vec();
                }
                "inherited_resource_extgstate" => {
                    resources.set("ExtGState", dictionary! { "GS1" => Dictionary::new() });
                    contents = b"/GS1 gs\n".to_vec();
                }
                "inherited_resource_font" => {
                    resources.set(
                        "Font",
                        dictionary! { "F1" => dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" } },
                    );
                    contents = b"BT\n/F1 12 Tf\nET\n".to_vec();
                }
                "inherited_resource_xobject" => {
                    let form = document.add_object(Stream::new(
                        dictionary! {
                            "Type" => "XObject",
                            "Subtype" => "Form",
                            "BBox" => vec![0.into(), 0.into(), 1.into(), 1.into()],
                            "Resources" => Dictionary::new(),
                        },
                        Vec::new(),
                    ));
                    resources.set("XObject", dictionary! { "X1" => form });
                    contents = b"/X1 Do\n".to_vec();
                }
                "inherited_resource_pattern" => {
                    let pattern = document.add_object(Stream::new(
                        dictionary! {
                            "Type" => "Pattern",
                            "PatternType" => 1,
                            "PaintType" => 1,
                            "TilingType" => 1,
                            "BBox" => vec![0.into(), 0.into(), 1.into(), 1.into()],
                            "XStep" => 1,
                            "YStep" => 1,
                            "Resources" => Dictionary::new(),
                        },
                        Vec::new(),
                    ));
                    resources.set("Pattern", dictionary! { "P1" => pattern });
                    contents = b"/Pattern cs\n/P1 scn\n".to_vec();
                }
                "inherited_resource_shading" => {
                    resources.set(
                        "Shading",
                        dictionary! { "S1" => dictionary! { "ShadingType" => 2, "ColorSpace" => "DeviceRGB", "Coords" => vec![0.into(), 0.into(), 1.into(), 1.into()], "Function" => dictionary! { "FunctionType" => 2, "Domain" => vec![0.into(), 1.into()], "C0" => vec![0.into(), 0.into(), 0.into()], "C1" => vec![1.into(), 1.into(), 1.into()], "N" => 1 }, "Extend" => vec![true.into(), true.into()] } },
                    );
                    contents = b"/S1 sh\n".to_vec();
                }
                "inherited_resource_properties" => {
                    resources.set("Properties", dictionary! { "Pr1" => Dictionary::new() });
                    contents = b"/Tag /Pr1 BDC\nEMC\n".to_vec();
                }
                _ => panic!("inconsistent inherited resource fixture {case}"),
            }
        }
        "extgstate_tr"
        | "direct_extgstate_tr"
        | "extgstate_tr_null"
        | "extgstate_tr_indirect_null"
        | "extgstate_tr2_default"
        | "extgstate_tr2_other"
        | "extgstate_tr2_null"
        | "indirect_extgstate_resource_dictionary"
        | "inherited_extgstate_tr"
        | "extgstate_ri_invalid"
        | "unused_extgstate_tr"
        | "unreferenced_extgstate_tr"
        | "extgstate_smask_none"
        | "extgstate_smask_other"
        | "extgstate_smask_dictionary"
        | "extgstate_smask_null"
        | "extgstate_smask_indirect_null"
        | "extgstate_bm_normal"
        | "extgstate_bm_compatible"
        | "extgstate_bm_multiply"
        | "extgstate_bm_invalid"
        | "extgstate_bm_null"
        | "extgstate_stroke_alpha_one"
        | "extgstate_stroke_alpha_zero"
        | "extgstate_fill_alpha_one"
        | "extgstate_fill_alpha_zero"
        | "unused_extgstate_transparency" => {
            let mut state = Dictionary::new();
            match case {
                "extgstate_tr"
                | "direct_extgstate_tr"
                | "indirect_extgstate_resource_dictionary"
                | "inherited_extgstate_tr"
                | "unused_extgstate_tr"
                | "unreferenced_extgstate_tr" => {
                    state.set("TR", "Identity");
                }
                "extgstate_tr_null" => state.set("TR", Object::Null),
                "extgstate_tr_indirect_null" => state.set("TR", document.add_object(Object::Null)),
                "extgstate_tr2_default" => state.set("TR2", "Default"),
                "extgstate_tr2_other" => state.set("TR2", "Identity"),
                "extgstate_tr2_null" => state.set("TR2", Object::Null),
                "extgstate_ri_invalid" => state.set("RI", "MaiIntent"),
                "extgstate_smask_none" => state.set("SMask", "None"),
                "extgstate_smask_other" | "unused_extgstate_transparency" => {
                    state.set("SMask", "Alpha")
                }
                "extgstate_smask_dictionary" => state.set("SMask", Dictionary::new()),
                "extgstate_smask_null" => state.set("SMask", Object::Null),
                "extgstate_smask_indirect_null" => {
                    state.set("SMask", document.add_object(Object::Null))
                }
                "extgstate_bm_normal" => state.set("BM", "Normal"),
                "extgstate_bm_compatible" => state.set("BM", "Compatible"),
                "extgstate_bm_multiply" => state.set("BM", "Multiply"),
                "extgstate_bm_invalid" => state.set("BM", "MaiBlendMode"),
                "extgstate_bm_null" => state.set("BM", Object::Null),
                "extgstate_stroke_alpha_one" => state.set("CA", 1),
                "extgstate_stroke_alpha_zero" => state.set("CA", 0),
                "extgstate_fill_alpha_one" => state.set("ca", 1),
                "extgstate_fill_alpha_zero" => state.set("ca", 0),
                _ => panic!("inconsistent graphics state fixture {case}"),
            }
            let state = if case == "direct_extgstate_tr" {
                Object::Dictionary(state)
            } else {
                Object::Reference(document.add_object(state))
            };
            if case != "unreferenced_extgstate_tr" {
                let states = dictionary! {"GS1" => state};
                if case == "indirect_extgstate_resource_dictionary" {
                    resources.set("ExtGState", document.add_object(states));
                } else {
                    resources.set("ExtGState", states);
                }
            }
            inherit_resources = case == "inherited_extgstate_tr";
            if !matches!(
                case,
                "unused_extgstate_tr"
                    | "unreferenced_extgstate_tr"
                    | "unused_extgstate_transparency"
            ) {
                contents = b"/GS1 gs\n".to_vec();
            }
        }
        "ri_standard" => {
            contents = b"/RelativeColorimetric ri\n/AbsoluteColorimetric ri\n/Perceptual ri\n/Saturation ri\n".to_vec();
        }
        "ri_invalid" => contents = b"/MaiIntent ri\n".to_vec(),
        "image_intent_valid"
        | "image_intent_invalid"
        | "explicit_mask_image_intent_invalid"
        | "soft_mask_image_intent_invalid" => {
            let intent = if case == "image_intent_valid" {
                "Perceptual"
            } else {
                "MaiIntent"
            };
            let image = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => if case == "explicit_mask_image_intent_invalid" { 1 } else { 8 },
                    "ColorSpace" => "DeviceRGB",
                    "Intent" => intent,
                },
                vec![0, 0, 0],
            ));
            let image_id = if matches!(
                case,
                "explicit_mask_image_intent_invalid" | "soft_mask_image_intent_invalid"
            ) {
                let key = if case == "explicit_mask_image_intent_invalid" {
                    "Mask"
                } else {
                    "SMask"
                };
                let mut dictionary = dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 8,
                    "ColorSpace" => "DeviceRGB",
                };
                dictionary.set(key, image);
                document.add_object(Stream::new(dictionary, vec![0, 0, 0]))
            } else {
                image
            };
            resources.set("XObject", dictionary! {"Im" => image_id});
            contents = b"/Im Do\n".to_vec();
        }
        "undefined_operator" => contents = b"1 2 MaiUnknown\n".to_vec(),
        "undefined_in_bx" => contents = b"BX\nMaiUnknown\nEX\n".to_vec(),
        "undefined_before_malformed_array" => contents = b"MaiUnknown\n[1 2\n".to_vec(),
        "malformed_string_before_undefined" => contents = b"(unterminated\nMaiUnknown\n".to_vec(),
        "unmatched_graphics_restore" => contents = b"Q\n".to_vec(),
        "gs_wrong_operand" | "gs_extra_operand" => {
            let state = document.add_object(dictionary! {"TR" => "Identity"});
            resources.set("ExtGState", dictionary! {"GS1" => state});
            contents = if case == "gs_wrong_operand" {
                b"1 gs\n".to_vec()
            } else {
                b"/GS1 1 gs\n".to_vec()
            };
        }
        "inline_image_lzw" => contents = b"BI /W 1 /H 1 /BPC 8 /F /LZW ID x EI\n".to_vec(),
        "inline_image_lzw_array" => {
            contents = b"BI /W 1 /H 1 /BPC 8 /Filter [/AHx /LZWDecode] ID x EI\n".to_vec()
        }
        "inline_image_lzw_escaped" => {
            contents = b"BI /W 1 /H 1 /BPC 8 /F /LZW#44ecode ID x EI\n".to_vec()
        }
        "inline_image_unterminated_lzw" => contents = b"BI /W 1 /H 1 /BPC 8 /F /LZW ID x".to_vec(),
        "inline_image_ascii_hex" => contents = b"BI /W 1 /H 1 /BPC 8 /F /AHx ID 00> EI\n".to_vec(),
        "inline_image_false_ei" => contents = b"BI /W 1 /H 1 /BPC 8 ID xEIx y EI\n".to_vec(),
        "inline_image_tokens_in_string" => contents = b"(BI /F /LZW ID x EI) Tj\n".to_vec(),
        "known_operators" => contents = b"q\n0 0 m\n1 1 l\nS\nQ\n".to_vec(),
        "graphics_state_nesting_28" => {
            contents = [vec![b'q'; 0], b"q\n".repeat(28), b"Q\n".repeat(28)].concat()
        }
        "graphics_state_nesting_29" => {
            contents = [vec![b'q'; 0], b"q\n".repeat(29), b"Q\n".repeat(29)].concat()
        }
        "undefined_form" | "unused_form_undefined" => {
            let form_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                },
                b"MaiUnknown\n".to_vec(),
            ));
            resources.set("XObject", dictionary! {"Fm" => form_id});
            if case == "undefined_form" {
                contents = b"/Fm Do\n".to_vec();
            }
        }
        "undefined_appearance" | "unused_appearance_undefined" => {
            let appearance = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                },
                b"MaiUnknown\n".to_vec(),
            ));
            let annotation = document.add_object(dictionary! {
                "Type" => "Annot",
                "Subtype" => "Text",
                "Rect" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                "F" => 4,
                "AP" => dictionary! {"N" => appearance},
            });
            if case == "undefined_appearance" {
                annotations.push(Object::Reference(annotation));
            }
        }
        "undefined_pattern" | "unused_pattern_undefined" => {
            let pattern = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "Pattern",
                    "PatternType" => 1,
                    "PaintType" => 1,
                    "TilingType" => 1,
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "XStep" => 10,
                    "YStep" => 10,
                },
                b"MaiUnknown\n".to_vec(),
            ));
            resources.set("Pattern", dictionary! {"P1" => pattern});
            if case == "undefined_pattern" {
                contents = b"/Pattern cs\n/P1 scn\n".to_vec();
            }
        }
        "malformed_pattern_leading_name"
        | "malformed_pattern_trailing_number"
        | "malformed_pattern_colorspace" => {
            let invalid = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "Pattern",
                    "PatternType" => 1,
                    "PaintType" => 1,
                    "TilingType" => 1,
                    "BBox" => vec![0.into(), 0.into(), 1.into(), 1.into()],
                    "XStep" => 1,
                    "YStep" => 1,
                },
                b"MaiUnknown\n".to_vec(),
            ));
            let valid = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "Pattern",
                    "PatternType" => 1,
                    "PaintType" => 1,
                    "TilingType" => 1,
                    "BBox" => vec![0.into(), 0.into(), 1.into(), 1.into()],
                    "XStep" => 1,
                    "YStep" => 1,
                },
                Vec::new(),
            ));
            resources.set(
                "Pattern",
                dictionary! {"Invalid" => invalid, "Valid" => valid},
            );
            contents = match case {
                "malformed_pattern_leading_name" => b"/Pattern cs\n/Invalid /Valid scn\n".to_vec(),
                "malformed_pattern_trailing_number" => b"/Pattern cs\n/Invalid 1 scn\n".to_vec(),
                _ => b"/Pattern 1 cs\n/Invalid scn\n".to_vec(),
            };
        }
        "shading_pattern_extgstate_tr"
        | "direct_shading_pattern_extgstate_tr"
        | "unused_shading_pattern_extgstate_tr" => {
            let pattern = dictionary! {
                "Type" => "Pattern",
                "PatternType" => 2,
                "Shading" => dictionary! {
                    "ShadingType" => 2,
                    "ColorSpace" => "DeviceRGB",
                    "Coords" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Function" => dictionary! {
                        "FunctionType" => 2,
                        "Domain" => vec![0.into(), 1.into()],
                        "C0" => vec![0.into(), 0.into(), 0.into()],
                        "C1" => vec![1.into(), 1.into(), 1.into()],
                        "N" => 1,
                    },
                    "Extend" => vec![Object::Boolean(true), Object::Boolean(true)],
                },
                "ExtGState" => dictionary! {"TR" => "Identity"},
            };
            let pattern = if case == "direct_shading_pattern_extgstate_tr" {
                Object::Dictionary(pattern)
            } else {
                Object::Reference(document.add_object(pattern))
            };
            resources.set("Pattern", dictionary! {"P1" => pattern});
            if case != "unused_shading_pattern_extgstate_tr" {
                contents = b"/Pattern cs\n/P1 scn\n".to_vec();
            }
        }
        "undefined_type3" | "unused_type3_undefined" => {
            let char_proc = document.add_object(Stream::new(
                Dictionary::new(),
                b"1000 0 d0\nMaiUnknown\n".to_vec(),
            ));
            let font = document.add_object(dictionary! {
                "Type" => "Font",
                "Subtype" => "Type3",
                "Name" => "T3",
                "FontBBox" => vec![0.into(), 0.into(), 1000.into(), 1000.into()],
                "FontMatrix" => vec![
                    0.001.into(), 0.into(), 0.into(), 0.001.into(), 0.into(), 0.into(),
                ],
                "CharProcs" => dictionary! {"g1" => char_proc},
                "Encoding" => dictionary! {
                    "Type" => "Encoding",
                    "Differences" => vec![65.into(), Object::Name(b"g1".to_vec())],
                },
                "FirstChar" => 65,
                "LastChar" => 65,
                "Widths" => vec![1000.into()],
            });
            resources.set("Font", dictionary! {"T3" => font});
            if case == "undefined_type3" {
                contents = b"BT\n/T3 12 Tf\n(A) Tj\nET\n".to_vec();
            }
        }
        "undefined_soft_mask_group" | "unused_soft_mask_group_undefined" => {
            let group = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Resources" => Dictionary::new(),
                    "Group" => dictionary! {
                        "Type" => "Group",
                        "S" => "Transparency",
                        "CS" => "DeviceGray",
                    },
                },
                b"MaiUnknown\n".to_vec(),
            ));
            let state = document.add_object(dictionary! {
                "Type" => "ExtGState",
                "SMask" => dictionary! {"S" => "Alpha", "G" => group},
            });
            resources.set("ExtGState", dictionary! {"GS1" => state});
            if case == "undefined_soft_mask_group" {
                contents = b"/GS1 gs\n".to_vec();
            }
        }
        "xobject_smask"
        | "unused_xobject_smask"
        | "xobject_smask_null"
        | "xobject_smask_indirect_null" => {
            let soft_mask = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 8,
                    "ColorSpace" => "DeviceGray",
                },
                vec![0],
            ));
            let soft_mask_value = if case == "xobject_smask_null" {
                Object::Null
            } else if case == "xobject_smask_indirect_null" {
                Object::Reference(document.add_object(Object::Null))
            } else {
                Object::Reference(soft_mask)
            };
            let image = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 8,
                    "ColorSpace" => "DeviceRGB",
                    "SMask" => soft_mask_value,
                },
                vec![0, 0, 0],
            ));
            resources.set("XObject", dictionary! {"Im" => image});
            if case != "unused_xobject_smask" {
                contents = b"/Im Do\n".to_vec();
            }
        }
        "page_transparency_group" | "page_nontransparency_group" => {
            page_group = Some(dictionary! {
                "Type" => "Group",
                "S" => if case == "page_transparency_group" {
                    Object::Name(b"Transparency".to_vec())
                } else {
                    Object::Name(b"MaiGroup".to_vec())
                },
            });
        }
        "form_transparency_group" | "unused_form_transparency_group" => {
            let form = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Group" => dictionary! {
                        "Type" => "Group",
                        "S" => "Transparency",
                    },
                },
                Vec::new(),
            ));
            resources.set("XObject", dictionary! {"Fm" => form});
            if case == "form_transparency_group" {
                contents = b"/Fm Do\n".to_vec();
            }
        }
        "extgstate_htp_present" => {
            let state = document.add_object(dictionary! {
                "Type" => "ExtGState",
                "HTP" => 1,
            });
            resources.set("ExtGState", dictionary! { "GS1" => state });
            contents = b"/GS1 gs\n".to_vec();
        }
        "halftone_type_invalid" | "halftone_name_present" => {
            let mut halftone = dictionary! {
                "HalftoneType" => if case == "halftone_type_invalid" { 2 } else { 5 },
            };
            if case == "halftone_name_present" {
                halftone.set("HalftoneName", Object::string_literal("TestHalftone"));
                halftone.set("Default", dictionary! { "HalftoneType" => 1 });
            }
            let state = document.add_object(dictionary! {
                "Type" => "ExtGState",
                "HT" => halftone,
            });
            resources.set("ExtGState", dictionary! { "GS1" => state });
            contents = b"/GS1 gs\n".to_vec();
        }
        "halftone_transfer_root_invalid"
        | "halftone_transfer_root_indirect_ht_invalid"
        | "halftone_transfer_unreferenced_invalid"
        | "halftone_transfer_unused_invalid"
        | "halftone_transfer_root_null"
        | "halftone_transfer_root_indirect_null"
        | "halftone_transfer_primary_invalid"
        | "halftone_transfer_spot_missing"
        | "halftone_transfer_default_present"
        | "halftone_transfer_spot_present" => {
            let transfer = Object::Name(b"Identity".to_vec());
            let child = |transfer_function: Option<Object>| {
                let mut halftone = dictionary! { "HalftoneType" => 1 };
                if let Some(transfer_function) = transfer_function {
                    halftone.set("TransferFunction", transfer_function);
                }
                Object::Dictionary(halftone)
            };
            let halftone = match case {
                "halftone_transfer_root_invalid" => {
                    dictionary! { "HalftoneType" => 1, "TransferFunction" => transfer }
                }
                "halftone_transfer_root_indirect_ht_invalid"
                | "halftone_transfer_unreferenced_invalid" => {
                    dictionary! { "HalftoneType" => 1, "TransferFunction" => transfer }
                }
                "halftone_transfer_unused_invalid" => {
                    dictionary! { "HalftoneType" => 1, "TransferFunction" => transfer }
                }
                "halftone_transfer_root_null" => {
                    dictionary! { "HalftoneType" => 1, "TransferFunction" => Object::Null }
                }
                "halftone_transfer_root_indirect_null" => dictionary! {
                    "HalftoneType" => 1,
                    "TransferFunction" => Object::Reference(document.add_object(Object::Null)),
                },
                "halftone_transfer_primary_invalid" => dictionary! {
                    "HalftoneType" => 5,
                    "Cyan" => child(Some(transfer)),
                },
                "halftone_transfer_spot_missing" => dictionary! {
                    "HalftoneType" => 5,
                    "Spot" => child(None),
                },
                "halftone_transfer_default_present" => dictionary! {
                    "HalftoneType" => 5,
                    "Default" => child(Some(transfer)),
                },
                "halftone_transfer_spot_present" => dictionary! {
                    "HalftoneType" => 5,
                    "Spot" => child(Some(transfer)),
                },
                _ => panic!("inconsistent halftone transfer fixture {case}"),
            };
            let halftone = if case == "halftone_transfer_root_indirect_ht_invalid" {
                Object::Reference(document.add_object(halftone))
            } else {
                Object::Dictionary(halftone)
            };
            let state =
                document.add_object(dictionary! { "Type" => "ExtGState", "HT" => halftone });
            if case != "halftone_transfer_unreferenced_invalid" {
                resources.set("ExtGState", dictionary! { "GS1" => state });
                if case != "halftone_transfer_unused_invalid" {
                    contents = b"/GS1 gs\n".to_vec();
                }
            }
        }
        _ => panic!("unknown graphics fixture case {case}"),
    }

    let contents_id = document.add_object(Stream::new(Dictionary::new(), contents));
    let mut page = dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => contents_id,
    };
    if !inherit_resources {
        page.set("Resources", resources.clone());
    }
    if !annotations.is_empty() {
        page.set("Annots", annotations);
    }
    if let Some(group) = page_group {
        page.set("Group", group);
    }
    let page_id = document.add_object(page);
    if inherit_resources {
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => 1,
                "Resources" => resources,
            }),
        );
    } else {
        wrap_pages(&mut document, pages_id, page_id);
    }
    let metadata_id = standard_metadata_stream(&mut document);
    let output_intents = single_profile_intent(
        &mut document,
        icc_header(*b"mntr", *b"RGB ", 2, 1),
        Some("GTS_PDFA1"),
    );
    if case == "extgstate_transparency_no_output_intent"
        && let Some(Object::Stream(stream)) = document.objects.get_mut(&metadata_id)
    {
        stream.content = String::from_utf8_lossy(BASE_XMP)
            .replace("pdfaid:part=\"1\"", "pdfaid:part=\"2\"")
            .into_bytes();
    }
    let mut catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
    };
    if !omit_output_intent {
        catalog.set("OutputIntents", output_intents.expect("output intent"));
    }
    let catalog_id = document.add_object(catalog);
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).expect("save graphics fixture");
    bytes
}

pub fn graphics_content_path_fixture(case: &str) -> Vec<u8> {
    let (source, violation) = ["form", "appearance", "pattern", "type3", "soft_mask"]
        .into_iter()
        .find_map(|source| {
            case.strip_prefix(&format!("path_{source}_"))
                .map(|violation| (source, violation))
        })
        .unwrap_or_else(|| panic!("unknown graphics content-path fixture case {case}"));

    let mut document = pdf_document();
    let pages_id = document.new_object_id();
    let mut inner_resources = Dictionary::new();
    let mut fallback_resources = Dictionary::new();
    let inner_contents = match violation {
        "undefined"
        | "invisible_undefined"
        | "malformed_text_show_undefined"
        | "missing_subtype_undefined"
        | "nested_state_undefined" => b"MaiUnknown\n".to_vec(),
        "missing_subtype_form_ref"
        | "image_subtype_bpc_16"
        | "appearance_and_painted_image_bpc_16" => Vec::new(),
        "extgstate_tr" => {
            let state = document.add_object(dictionary! {"TR" => "Identity"});
            inner_resources.set("ExtGState", dictionary! {"GS1" => state});
            b"/GS1 gs\n".to_vec()
        }
        "halftone_transfer_invalid" => {
            let state = document.add_object(dictionary! {
                "HT" => dictionary! {
                    "HalftoneType" => 1,
                    "TransferFunction" => "Identity",
                },
            });
            inner_resources.set("ExtGState", dictionary! {"GS1" => state});
            b"/GS1 gs\n".to_vec()
        }
        "fallback_extgstate_tr" | "missing_resources_fallback_extgstate_tr" => {
            let state = document.add_object(dictionary! {"TR" => "Identity"});
            fallback_resources.set("ExtGState", dictionary! {"GS1" => state});
            b"/GS1 gs\n".to_vec()
        }
        "parent_only_extgstate_tr"
        | "missing_resources_parent_extgstate_tr"
        | "empty_resources_parent_extgstate_tr" => b"/GS1 gs\n".to_vec(),
        "image_bpc_16" => {
            let image = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 16,
                    "ColorSpace" => "DeviceRGB",
                },
                vec![0, 0, 0],
            ));
            inner_resources.set("XObject", dictionary! {"Im" => image});
            b"/Im Do\n".to_vec()
        }
        "inherited_pattern_undefined" => {
            let pattern = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "Pattern",
                    "PatternType" => 1,
                    "PaintType" => 1,
                    "TilingType" => 1,
                    "BBox" => vec![0.into(), 0.into(), 1.into(), 1.into()],
                    "XStep" => 1,
                    "YStep" => 1,
                },
                b"MaiUnknown\n".to_vec(),
            ));
            inner_resources.set("Pattern", dictionary! {"Inner" => pattern});
            b"/Inner scn\n".to_vec()
        }
        "nesting_29" => [b"q\n".repeat(29), b"Q\n".repeat(29)].concat(),
        "inline_lzw" => b"BI /W 1 /H 1 /BPC 8 /F /LZW ID x EI\n".to_vec(),
        "invalid_intent" => b"/MaiIntent ri\n".to_vec(),
        _ => panic!("unknown graphics content-path violation {violation}"),
    };

    let mut page_resources = fallback_resources;
    let mut page_contents = Vec::new();
    let mut annotations = Vec::new();
    match source {
        "form" => {
            let form = if matches!(
                violation,
                "parent_only_extgstate_tr" | "missing_resources_parent_extgstate_tr"
            ) {
                let mut inner_dictionary = dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                };
                if violation == "parent_only_extgstate_tr" {
                    inner_dictionary.set("Resources", Dictionary::new());
                }
                let inner = document.add_object(Stream::new(inner_dictionary, inner_contents));
                let state = document.add_object(dictionary! {"TR" => "Identity"});
                document.add_object(Stream::new(
                    dictionary! {
                        "Type" => "XObject",
                        "Subtype" => "Form",
                        "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                        "Resources" => dictionary! {
                            "XObject" => dictionary! {"Inner" => inner},
                            "ExtGState" => dictionary! {"GS1" => state},
                        },
                    },
                    b"/Inner Do\n".to_vec(),
                ))
            } else {
                let mut dictionary = dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                };
                if violation != "missing_resources_fallback_extgstate_tr" {
                    dictionary.set("Resources", inner_resources);
                }
                document.add_object(Stream::new(dictionary, inner_contents))
            };
            page_resources.set("XObject", dictionary! {"Container" => form});
            page_contents = b"/Container Do\n".to_vec();
        }
        "appearance" => {
            let mut dictionary = dictionary! {
                "Type" => "XObject",
                "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
            };
            if !matches!(
                violation,
                "missing_subtype_undefined" | "missing_subtype_form_ref"
            ) {
                dictionary.set("Subtype", "Form");
            }
            if violation == "missing_subtype_form_ref" {
                dictionary.set("Ref", Dictionary::new());
            }
            if matches!(
                violation,
                "image_subtype_bpc_16" | "appearance_and_painted_image_bpc_16"
            ) {
                dictionary.set("Subtype", "Image");
                dictionary.set("Width", 1);
                dictionary.set("Height", 1);
                dictionary.set("BitsPerComponent", 16);
                dictionary.set("ColorSpace", "DeviceRGB");
            }
            if violation != "missing_resources_fallback_extgstate_tr" {
                dictionary.set("Resources", inner_resources);
            }
            let appearance = document.add_object(Stream::new(dictionary, inner_contents));
            let normal_appearance = if violation == "nested_state_undefined" {
                Object::Dictionary(dictionary! {
                    "Outer" => dictionary! {"Inner" => appearance},
                })
            } else {
                Object::Reference(appearance)
            };
            let annotation = document.add_object(dictionary! {
                "Type" => "Annot",
                "Subtype" => "Text",
                "Rect" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                "F" => 4,
                "AP" => dictionary! {"N" => normal_appearance},
            });
            annotations.push(Object::Reference(annotation));
            if violation == "appearance_and_painted_image_bpc_16" {
                page_resources.set("XObject", dictionary! {"Im" => appearance});
                page_contents = b"/Im Do\n".to_vec();
            }
        }
        "pattern" => {
            if matches!(
                violation,
                "missing_resources_parent_extgstate_tr" | "empty_resources_parent_extgstate_tr"
            ) {
                let mut pattern_dictionary = dictionary! {
                    "Type" => "Pattern",
                    "PatternType" => 1,
                    "PaintType" => 1,
                    "TilingType" => 1,
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "XStep" => 10,
                    "YStep" => 10,
                };
                if violation == "empty_resources_parent_extgstate_tr" {
                    pattern_dictionary.set("Resources", Dictionary::new());
                }
                let pattern = document.add_object(Stream::new(pattern_dictionary, inner_contents));
                let state = document.add_object(dictionary! {"TR" => "Identity"});
                let outer = document.add_object(Stream::new(
                    dictionary! {
                        "Type" => "XObject",
                        "Subtype" => "Form",
                        "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                        "Resources" => dictionary! {
                            "Pattern" => dictionary! {"Container" => pattern},
                            "ExtGState" => dictionary! {"GS1" => state},
                        },
                    },
                    b"/Pattern cs\n/Container scn\n".to_vec(),
                ));
                page_resources.set("XObject", dictionary! {"Outer" => outer});
                page_contents = b"/Outer Do\n".to_vec();
            } else {
                let mut dictionary = dictionary! {
                    "Type" => "Pattern",
                    "PatternType" => 1,
                    "PaintType" => 1,
                    "TilingType" => 1,
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "XStep" => 10,
                    "YStep" => 10,
                };
                if violation != "missing_resources_fallback_extgstate_tr" {
                    dictionary.set("Resources", inner_resources);
                }
                let pattern = document.add_object(Stream::new(dictionary, inner_contents));
                page_resources.set("Pattern", dictionary! {"Container" => pattern});
                page_contents = b"/Pattern cs\n/Container scn\n".to_vec();
            }
        }
        "type3" => {
            let char_proc = document.add_object(Stream::new(
                Dictionary::new(),
                [b"1000 0 d0\n".as_slice(), inner_contents.as_slice()].concat(),
            ));
            let mut font_dictionary = dictionary! {
                "Type" => "Font",
                "Subtype" => "Type3",
                "Name" => "T3",
                "FontBBox" => vec![0.into(), 0.into(), 1000.into(), 1000.into()],
                "FontMatrix" => vec![
                    0.001.into(), 0.into(), 0.into(), 0.001.into(), 0.into(), 0.into(),
                ],
                "CharProcs" => dictionary! {"g1" => char_proc},
                "Encoding" => dictionary! {
                    "Type" => "Encoding",
                    "Differences" => vec![65.into(), Object::Name(b"g1".to_vec())],
                },
                "FirstChar" => 65,
                "LastChar" => 65,
                "Widths" => vec![1000.into()],
            };
            if violation == "empty_resources_parent_extgstate_tr" {
                font_dictionary.set("Resources", Dictionary::new());
            } else if !matches!(
                violation,
                "missing_resources_parent_extgstate_tr" | "missing_resources_fallback_extgstate_tr"
            ) {
                font_dictionary.set("Resources", inner_resources);
            }
            let font = document.add_object(font_dictionary);
            let text_contents = if violation == "invisible_undefined" {
                b"BT\n/T3 12 Tf\n3 Tr\n(A) Tj\nET\n".to_vec()
            } else if violation == "malformed_text_show_undefined" {
                b"BT\n/T3 12 Tf\n(A) 1 Tj\nET\n".to_vec()
            } else if violation == "inherited_pattern_undefined" {
                b"/Pattern cs\nBT\n/T3 12 Tf\n(A) Tj\nET\n".to_vec()
            } else {
                b"BT\n/T3 12 Tf\n(A) Tj\nET\n".to_vec()
            };
            if matches!(
                violation,
                "missing_resources_parent_extgstate_tr" | "empty_resources_parent_extgstate_tr"
            ) {
                let state = document.add_object(dictionary! {"TR" => "Identity"});
                let outer = document.add_object(Stream::new(
                    dictionary! {
                        "Type" => "XObject",
                        "Subtype" => "Form",
                        "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                        "Resources" => dictionary! {
                            "Font" => dictionary! {"T3" => font},
                            "ExtGState" => dictionary! {"GS1" => state},
                        },
                    },
                    text_contents,
                ));
                page_resources.set("XObject", dictionary! {"Outer" => outer});
                page_contents = b"/Outer Do\n".to_vec();
            } else {
                page_resources.set("Font", dictionary! {"T3" => font});
                page_contents = text_contents;
            }
        }
        "soft_mask" => {
            let group = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "Resources" => inner_resources,
                    "Group" => dictionary! {
                        "Type" => "Group",
                        "S" => "Transparency",
                        "CS" => "DeviceGray",
                    },
                },
                inner_contents,
            ));
            let state = document.add_object(dictionary! {
                "Type" => "ExtGState",
                "SMask" => dictionary! {"S" => "Alpha", "G" => group},
            });
            page_resources.set("ExtGState", dictionary! {"GS1" => state});
            page_contents = b"/GS1 gs\n".to_vec();
        }
        _ => panic!("inconsistent transparency fixture {case}"),
    }

    let contents_id = document.add_object(Stream::new(Dictionary::new(), page_contents));
    let mut page = dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => page_resources,
        "Contents" => contents_id,
    };
    if !annotations.is_empty() {
        page.set("Annots", annotations);
    }
    let page_id = document.add_object(page);
    wrap_pages(&mut document, pages_id, page_id);
    let metadata_id = standard_metadata_stream(&mut document);
    let output_intents = single_profile_intent(
        &mut document,
        icc_header(*b"mntr", *b"RGB ", 2, 1),
        Some("GTS_PDFA1"),
    );
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "OutputIntents" => output_intents.expect("output intent"),
    });
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save graphics content-path fixture");
    bytes
}

pub fn annotation_fixture(case: &str) -> Vec<u8> {
    let mut document = pdf_document();
    let pages_id = document.new_object_id();
    let appearance_stream = document.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
        },
        Vec::new(),
    ));
    let mut annotation = dictionary! {
        "Type" => "Annot",
        "Subtype" => "Text",
        "Rect" => vec![0.into(), 0.into(), 10.into(), 10.into()],
        "F" => 4,
    };
    let mut include_annotation = true;
    let mut direct_annotation = false;
    let mut direct_page = false;
    let mut stream_annotation = false;
    let mut output_color_space = Some(*b"RGB ");

    match case {
        "baseline" | "opacity_absent" | "appearance_absent" => {}
        "subtype_widget" => annotation.set("Subtype", "Widget"),
        "subtype_trapnet" => annotation.set("Subtype", "TrapNet"),
        "subtype_file_attachment" => annotation.set("Subtype", "FileAttachment"),
        "subtype_unknown" | "unreferenced_invalid_annotation" => {
            annotation.set("Subtype", "MaiAnnot")
        }
        "subtype_missing" => {
            annotation.remove(b"Subtype");
        }
        "subtype_indirect" => {
            let subtype = document.add_object(Object::Name(b"FileAttachment".to_vec()));
            annotation.set("Subtype", subtype);
        }
        "stream_annotation_ignored" => {
            annotation.set("Subtype", "FileAttachment");
            annotation.remove(b"F");
            stream_annotation = true;
        }
        "direct_invalid_annotation" => {
            annotation.set("Subtype", "MaiAnnot");
            direct_annotation = true;
        }
        // Confirmed against veraPDF 1.30.2: a Page dictionary embedded
        // directly (not as an indirect reference) in the page tree's Kids
        // array is still walked and its annotations validated.
        "direct_page_invalid_annotation" => {
            annotation.set("Subtype", "MaiAnnot");
            direct_page = true;
        }
        "opacity_one" => annotation.set("CA", 1),
        "opacity_zero" => annotation.set("CA", 0),
        "opacity_zero_indirect" => {
            let opacity = document.add_object(Object::Integer(0));
            annotation.set("CA", opacity);
        }
        "opacity_wrong_type" => annotation.set("CA", "Opaque"),
        "flags_missing" => {
            annotation.remove(b"F");
        }
        "flags_not_printable" => annotation.set("F", 0),
        "flags_invisible" => annotation.set("F", 5),
        "flags_hidden" => annotation.set("F", 6),
        "flags_no_view" => annotation.set("F", 36),
        "flags_invisible_indirect" => {
            let flags = document.add_object(Object::Integer(5));
            annotation.set("F", flags);
        }
        "color_c_rgb" => annotation.set("C", vec![1.into(), 0.into(), 0.into()]),
        "color_c_null" => {
            annotation.set("C", Object::Null);
            output_color_space = Some(*b"CMYK");
        }
        "color_c_wrong_type" => {
            annotation.set("C", 42);
            output_color_space = Some(*b"CMYK");
        }
        "color_c_indirect_array" => {
            let color = document.add_object(Object::Array(vec![1.into(), 0.into(), 0.into()]));
            annotation.set("C", color);
            output_color_space = Some(*b"CMYK");
        }
        "color_ic_rgb" => annotation.set("IC", vec![1.into(), 0.into(), 0.into()]),
        "color_c_cmyk" => {
            annotation.set("C", vec![1.into(), 0.into(), 0.into()]);
            output_color_space = Some(*b"CMYK");
        }
        "color_ic_without_output" => {
            annotation.set("IC", vec![1.into(), 0.into(), 0.into()]);
            output_color_space = None;
        }
        "no_color_cmyk" => output_color_space = Some(*b"CMYK"),
        "appearance_n_stream" => annotation.set("AP", dictionary! {"N" => appearance_stream}),
        "appearance_n_dictionary" => annotation.set(
            "AP",
            dictionary! {
                "N" => dictionary! {
                    "On" => appearance_stream,
                },
            },
        ),
        "appearance_n_and_r" => annotation.set(
            "AP",
            dictionary! {
                "N" => appearance_stream,
                "R" => appearance_stream,
            },
        ),
        "appearance_empty" => annotation.set("AP", Dictionary::new()),
        "appearance_wrong_type" => annotation.set("AP", 42),
        "widget_button_dictionary" | "widget_button_empty_dictionary" => {
            annotation.set("Subtype", "Widget");
            annotation.set("FT", "Btn");
            annotation.set(
                "AP",
                dictionary! {
                    "N" => if case == "widget_button_dictionary" {
                        Object::Dictionary(dictionary! {"Yes" => appearance_stream})
                    } else {
                        Object::Dictionary(Dictionary::new())
                    },
                },
            );
        }
        "widget_button_stream" => {
            annotation.set("Subtype", "Widget");
            annotation.set("FT", "Btn");
            annotation.set("AP", dictionary! {"N" => appearance_stream});
        }
        "widget_button_scalar_state" => {
            annotation.set("Subtype", "Widget");
            annotation.set("FT", "Btn");
            annotation.set("AP", dictionary! {"N" => dictionary! {"Yes" => 42}});
        }
        "widget_text_stream" => {
            annotation.set("Subtype", "Widget");
            annotation.set("FT", "Tx");
            annotation.set("AP", dictionary! {"N" => appearance_stream});
        }
        "widget_inherited_button_dictionary" => {
            let parent = document.add_object(dictionary! {
                "FT" => "Btn",
            });
            annotation.set("Subtype", "Widget");
            annotation.set("Parent", parent);
            annotation.set(
                "AP",
                dictionary! {
                    "N" => dictionary! {"Yes" => appearance_stream},
                },
            );
        }
        "widget_inherited_button_stream_parent" => {
            let parent = document.add_object(Stream::new(dictionary! {"FT" => "Btn"}, Vec::new()));
            annotation.set("Subtype", "Widget");
            annotation.set("Parent", parent);
            annotation.set(
                "AP",
                dictionary! {
                    "N" => dictionary! {"Yes" => appearance_stream},
                },
            );
        }
        _ => panic!("unknown annotation fixture case {case}"),
    }

    if case == "unreferenced_invalid_annotation" {
        include_annotation = false;
    }
    let annotation_object = if stream_annotation {
        Object::Reference(document.add_object(Stream::new(annotation, Vec::new())))
    } else if direct_annotation {
        Object::Dictionary(annotation)
    } else {
        Object::Reference(document.add_object(annotation))
    };
    let contents_id = document.add_object(Stream::new(Dictionary::new(), Vec::new()));
    let mut page = dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => Dictionary::new(),
        "Contents" => contents_id,
    };
    if include_annotation {
        page.set("Annots", vec![annotation_object]);
    }
    if direct_page {
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Dictionary(page)],
                "Count" => 1,
            }),
        );
    } else {
        let page_id = document.add_object(page);
        wrap_pages(&mut document, pages_id, page_id);
    }
    let metadata_id = standard_metadata_stream(&mut document);
    let mut catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
    };
    if let Some(color_space) = output_color_space {
        let output_intents = single_profile_intent(
            &mut document,
            icc_header(*b"mntr", color_space, 2, 1),
            Some("GTS_PDFA1"),
        );
        catalog.set("OutputIntents", output_intents.expect("output intent"));
    }
    let catalog_id = document.add_object(catalog);
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save annotation fixture");
    bytes
}

pub fn action_fixture(case: &str) -> Vec<u8> {
    let mut document = pdf_document();
    let pages_id = document.new_object_id();
    let widget_appearance = document.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
        },
        Vec::new(),
    ));
    let mut catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    };
    let mut page = dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => Dictionary::new(),
    };
    let mut annotations = Vec::new();
    let mut fields = Vec::new();
    let mut stream_acro_form = false;

    match case {
        "baseline" => {}
        "allowed_goto" => catalog.set("OpenAction", action("GoTo")),
        "allowed_gotor" => catalog.set("OpenAction", action("GoToR")),
        "allowed_thread" => catalog.set("OpenAction", action("Thread")),
        "allowed_uri" => catalog.set("OpenAction", action("URI")),
        "allowed_named" => {
            let mut named = action_dictionary("Named");
            named.set("N", "NextPage");
            catalog.set("OpenAction", named);
        }
        "allowed_submit_form" => catalog.set("OpenAction", action("SubmitForm")),
        "gotor_action_with_ef_file_spec" => {
            let mut goto_r = action_dictionary("GoToR");
            goto_r.set("F", file_spec_with_ef("test.txt"));
            catalog.set("OpenAction", goto_r);
        }
        "submit_form_action_with_ef_file_spec" => {
            let mut submit_form = action_dictionary("SubmitForm");
            submit_form.set("F", file_spec_with_ef("https://example.com/submit"));
            catalog.set("OpenAction", submit_form);
        }
        "gotor_action_without_ef_file_spec" => {
            let mut goto_r = action_dictionary("GoToR");
            goto_r.set(
                "F",
                dictionary! {"Type" => "Filespec", "F" => Object::string_literal("test.txt")},
            );
            catalog.set("OpenAction", goto_r);
        }
        "open_javascript" | "open_javascript_indirect" => {
            let javascript = action_dictionary("JavaScript");
            if case == "open_javascript_indirect" {
                catalog.set("OpenAction", document.add_object(javascript));
            } else {
                catalog.set("OpenAction", javascript);
            }
        }
        "open_missing_subtype" => catalog.set("OpenAction", Dictionary::new()),
        "open_wrong_subtype_type" => {
            catalog.set(
                "OpenAction",
                dictionary! {"S" => Object::string_literal("GoTo")},
            );
        }
        "open_indirect_subtype" => {
            let subtype = document.add_object(Object::Name(b"JavaScript".to_vec()));
            catalog.set("OpenAction", dictionary! {"S" => subtype});
        }
        "open_destination_array" => {
            catalog.set(
                "OpenAction",
                vec![Object::Reference(pages_id), Object::Name(b"Fit".to_vec())],
            );
        }
        "unreferenced_javascript" => {
            document.add_object(action_dictionary("JavaScript"));
        }
        "page_additional_action" => {
            page.set("AA", dictionary! {"O" => action("JavaScript")});
        }
        "page_unknown_additional_action" => {
            page.set("AA", dictionary! {"X" => action("JavaScript")});
        }
        "annotation_action" => {
            let mut annotation = valid_annotation("Text");
            annotation.set("A", action("JavaScript"));
            annotations.push(Object::Reference(document.add_object(annotation)));
        }
        "annotation_additional_action" => {
            let mut annotation = valid_annotation("Text");
            annotation.set("AA", dictionary! {"E" => action("JavaScript")});
            annotations.push(Object::Reference(document.add_object(annotation)));
        }
        "annotation_unknown_additional_action" => {
            let mut annotation = valid_annotation("Text");
            annotation.set("AA", dictionary! {"K" => action("JavaScript")});
            annotations.push(Object::Reference(document.add_object(annotation)));
        }
        "outline_action" => {
            let outlines_id = document.new_object_id();
            let outline_id = document.add_object(dictionary! {
                "Title" => Object::string_literal("Page outline"),
                "Parent" => outlines_id,
                "A" => action("JavaScript"),
            });
            document.objects.insert(
                outlines_id,
                Object::Dictionary(dictionary! {
                    "Type" => "Outlines",
                    "First" => outline_id,
                    "Last" => outline_id,
                    "Count" => 1,
                }),
            );
            catalog.set("Outlines", outlines_id);
        }
        "outline_stream_action" | "outline_stream_node_action" => {
            let outlines_id = document.new_object_id();
            let action_value = if case == "outline_stream_action" {
                Object::Reference(
                    document.add_object(Stream::new(dictionary! {"S" => "JavaScript"}, Vec::new())),
                )
            } else {
                action("JavaScript")
            };
            let outline_dictionary = dictionary! {
                "Title" => Object::string_literal("Page outline"),
                "Parent" => outlines_id,
                "A" => action_value,
            };
            let outline_id = if case == "outline_stream_node_action" {
                document.add_object(Stream::new(outline_dictionary, Vec::new()))
            } else {
                document.add_object(outline_dictionary)
            };
            document.objects.insert(
                outlines_id,
                Object::Dictionary(dictionary! {
                    "Type" => "Outlines",
                    "First" => outline_id,
                    "Last" => outline_id,
                    "Count" => 1,
                }),
            );
            catalog.set("Outlines", outlines_id);
        }
        "next_action" => {
            let mut first = action_dictionary("GoTo");
            first.set("Next", action("JavaScript"));
            catalog.set("OpenAction", first);
        }
        "next_action_array" => {
            let mut first = action_dictionary("GoTo");
            first.set("Next", vec![action("URI"), action("JavaScript"), 42.into()]);
            catalog.set("OpenAction", first);
        }
        "next_stream_action_ignored" => {
            let next =
                document.add_object(Stream::new(dictionary! {"S" => "JavaScript"}, Vec::new()));
            let mut first = action_dictionary("GoTo");
            first.set("Next", next);
            catalog.set("OpenAction", first);
        }
        "next_action_cycle" => {
            let action_id = document.new_object_id();
            document.objects.insert(
                action_id,
                Object::Dictionary(dictionary! {
                    "S" => "GoTo",
                    "Next" => action_id,
                }),
            );
            catalog.set("OpenAction", action_id);
        }
        "named_next_page" => {
            let mut named = action_dictionary("Named");
            named.set("N", "NextPage");
            catalog.set("OpenAction", named);
        }
        "named_prev_page" => {
            let mut named = action_dictionary("Named");
            named.set("N", "PrevPage");
            catalog.set("OpenAction", named);
        }
        "named_first_page" => {
            let mut named = action_dictionary("Named");
            named.set("N", "FirstPage");
            catalog.set("OpenAction", named);
        }
        "named_last_page" => {
            let mut named = action_dictionary("Named");
            named.set("N", "LastPage");
            catalog.set("OpenAction", named);
        }
        "named_forbidden" => {
            let mut named = action_dictionary("Named");
            named.set("N", "Print");
            catalog.set("OpenAction", named);
        }
        "named_missing" => catalog.set("OpenAction", action("Named")),
        "named_wrong_type" => {
            let mut named = action_dictionary("Named");
            named.set("N", Object::string_literal("NextPage"));
            catalog.set("OpenAction", named);
        }
        "named_indirect_forbidden" => {
            let name = document.add_object(Object::Name(b"Print".to_vec()));
            let mut named = action_dictionary("Named");
            named.set("N", name);
            catalog.set("OpenAction", named);
        }
        "non_named_with_forbidden_n" => {
            let mut uri = action_dictionary("URI");
            uri.set("N", "Print");
            catalog.set("OpenAction", uri);
        }
        "widget_action" | "widget_action_wrong_type" => {
            let mut widget = valid_annotation("Widget");
            widget.set("FT", "Tx");
            widget.set("AP", dictionary! {"N" => widget_appearance});
            widget.set(
                "A",
                if case == "widget_action" {
                    action("URI")
                } else {
                    42.into()
                },
            );
            annotations.push(Object::Reference(document.add_object(widget)));
        }
        "widget_a_null" => {
            let mut widget = valid_annotation("Widget");
            widget.set("FT", "Tx");
            widget.set("AP", dictionary! {"N" => widget_appearance});
            widget.set("A", Object::Null);
            annotations.push(Object::Reference(document.add_object(widget)));
        }
        "widget_indirect_subtype_action" => {
            let subtype = document.add_object(Object::Name(b"Widget".to_vec()));
            let mut widget = valid_annotation("Text");
            widget.set("Subtype", subtype);
            widget.set("FT", "Tx");
            widget.set("AP", dictionary! {"N" => widget_appearance});
            widget.set("A", action("URI"));
            annotations.push(Object::Reference(document.add_object(widget)));
        }
        "stream_widget_action_ignored" => {
            let mut widget = valid_annotation("Widget");
            widget.remove(b"F");
            widget.set("FT", "Tx");
            widget.set("AP", dictionary! {"N" => widget_appearance});
            widget.set("A", action("URI"));
            annotations.push(Object::Reference(
                document.add_object(Stream::new(widget, Vec::new())),
            ));
        }
        "widget_additional_actions" => {
            let mut widget = valid_annotation("Widget");
            widget.set("FT", "Tx");
            widget.set("AP", dictionary! {"N" => widget_appearance});
            widget.set("AA", Dictionary::new());
            annotations.push(Object::Reference(document.add_object(widget)));
        }
        "widget_aa_null" => {
            let mut widget = valid_annotation("Widget");
            widget.set("FT", "Tx");
            widget.set("AP", dictionary! {"N" => widget_appearance});
            widget.set("AA", Object::Null);
            annotations.push(Object::Reference(document.add_object(widget)));
        }
        "widget_additional_javascript" => {
            let mut widget = valid_annotation("Widget");
            widget.set("FT", "Tx");
            widget.set("AP", dictionary! {"N" => widget_appearance});
            widget.set("AA", dictionary! {"E" => action("JavaScript")});
            annotations.push(Object::Reference(document.add_object(widget)));
        }
        "text_additional_actions" => {
            let mut annotation = valid_annotation("Text");
            annotation.set("AA", Dictionary::new());
            annotations.push(Object::Reference(document.add_object(annotation)));
        }
        "field_additional_actions" | "top_field_without_t" | "direct_field_additional_actions" => {
            let mut field = dictionary! {
                "T" => Object::string_literal("field"),
                "FT" => "Tx",
                "AA" => Dictionary::new(),
            };
            if case == "top_field_without_t" {
                field.remove(b"T");
            }
            let field_object = if case == "direct_field_additional_actions" {
                Object::Dictionary(field)
            } else {
                Object::Reference(document.add_object(field))
            };
            fields.push(field_object);
        }
        "field_aa_null" => {
            fields.push(Object::Reference(document.add_object(dictionary! {
                "T" => Object::string_literal("field"),
                "FT" => "Tx",
                "AA" => Object::Null,
            })));
        }
        "field_additional_javascript" => {
            fields.push(Object::Reference(document.add_object(dictionary! {
                "T" => Object::string_literal("field"),
                "FT" => "Tx",
                "AA" => dictionary! {"K" => action("JavaScript")},
            })));
        }
        "stream_acroform_field_additional_actions" => {
            fields.push(Object::Reference(document.add_object(dictionary! {
                "T" => Object::string_literal("field"),
                "FT" => "Tx",
                "AA" => Dictionary::new(),
            })));
            stream_acro_form = true;
        }
        "stream_field_additional_actions" => {
            let field = Stream::new(
                dictionary! {
                    "T" => Object::string_literal("field"),
                    "FT" => "Tx",
                    "AA" => Dictionary::new(),
                },
                Vec::new(),
            );
            fields.push(Object::Reference(document.add_object(field)));
        }
        "stream_field_aa_javascript" => {
            let aa = document.add_object(Stream::new(
                dictionary! {"K" => action("JavaScript")},
                Vec::new(),
            ));
            fields.push(Object::Reference(document.add_object(dictionary! {
                "T" => Object::string_literal("field"),
                "FT" => "Tx",
                "AA" => aa,
            })));
        }
        "child_field_additional_actions" | "child_without_t" => {
            let mut child = dictionary! {
                "T" => Object::string_literal("child"),
                "AA" => Dictionary::new(),
            };
            if case == "child_without_t" {
                child.remove(b"T");
            }
            let child_id = document.add_object(child);
            fields.push(Object::Reference(document.add_object(dictionary! {
                "T" => Object::string_literal("parent"),
                "FT" => "Tx",
                "Kids" => vec![Object::Reference(child_id)],
            })));
        }
        "stream_child_field_additional_actions" => {
            let child_id = document.add_object(Stream::new(
                dictionary! {
                    "T" => Object::string_literal("child"),
                    "AA" => Dictionary::new(),
                },
                Vec::new(),
            ));
            fields.push(Object::Reference(document.add_object(dictionary! {
                "T" => Object::string_literal("parent"),
                "FT" => "Tx",
                "Kids" => vec![Object::Reference(child_id)],
            })));
        }
        "unnamed_child_reused_as_top_field" => {
            let reused = document.add_object(dictionary! {"AA" => Dictionary::new()});
            fields.push(Object::Reference(document.add_object(dictionary! {
                "T" => Object::string_literal("parent"),
                "FT" => "Tx",
                "Kids" => vec![Object::Reference(reused)],
            })));
            fields.push(Object::Reference(reused));
        }
        "field_cycle" => {
            let field_id = document.new_object_id();
            document.objects.insert(
                field_id,
                Object::Dictionary(dictionary! {
                    "T" => Object::string_literal("field"),
                    "FT" => "Tx",
                    "AA" => Dictionary::new(),
                    "Kids" => vec![Object::Reference(field_id)],
                }),
            );
            fields.push(Object::Reference(field_id));
        }
        "unreferenced_field_additional_actions" => {
            document.add_object(dictionary! {
                "T" => Object::string_literal("unreferenced"),
                "AA" => Dictionary::new(),
            });
        }
        "combined_widget_field_actions" => {
            let mut widget = valid_annotation("Widget");
            widget.set("T", Object::string_literal("combined"));
            widget.set("FT", "Tx");
            widget.set("AP", dictionary! {"N" => widget_appearance});
            widget.set("A", action("URI"));
            widget.set("AA", Dictionary::new());
            let widget_id = document.add_object(widget);
            annotations.push(Object::Reference(widget_id));
            fields.push(Object::Reference(widget_id));
        }
        "catalog_additional_actions" => catalog.set("AA", Dictionary::new()),
        "catalog_aa_null" => catalog.set("AA", Object::Null),
        "catalog_additional_javascript" => {
            catalog.set("AA", dictionary! {"WC" => action("JavaScript")});
        }
        "catalog_unknown_additional_action" => {
            catalog.set("AA", dictionary! {"X" => action("JavaScript")});
        }
        "catalog_additional_actions_wrong_type" => catalog.set("AA", 42),
        _ => panic!("unknown action fixture case {case}"),
    }

    if !fields.is_empty() {
        let acro_form = dictionary! {"Fields" => fields};
        if stream_acro_form {
            let acro_form = document.add_object(Stream::new(acro_form, Vec::new()));
            catalog.set("AcroForm", acro_form);
        } else {
            catalog.set("AcroForm", acro_form);
        }
    }
    if !annotations.is_empty() {
        page.set("Annots", annotations);
    }
    let contents_id = document.add_object(Stream::new(Dictionary::new(), Vec::new()));
    page.set("Contents", contents_id);
    let page_id = document.add_object(page);
    wrap_pages(&mut document, pages_id, page_id);
    let metadata_id = standard_metadata_stream(&mut document);
    catalog.set("Metadata", metadata_id);
    let output_intents = single_profile_intent(
        &mut document,
        icc_header(*b"mntr", *b"RGB ", 2, 1),
        Some("GTS_PDFA1"),
    );
    catalog.set("OutputIntents", output_intents.expect("output intent"));
    let catalog_id = document.add_object(catalog);
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).expect("save action fixture");
    bytes
}

pub fn form_fixture(case: &str) -> Vec<u8> {
    let mut document = pdf_document();
    let pages_id = document.new_object_id();
    let appearance_stream = document.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
        },
        Vec::new(),
    ));
    let mut widget = valid_annotation("Widget");
    widget.set("FT", "Tx");
    widget.set("T", Object::string_literal("field"));
    widget.set("AP", dictionary! {"N" => appearance_stream});
    let mut include_on_page = true;
    let mut include_as_field = true;
    let mut direct_widget = false;
    let mut stream_widget = false;
    let mut include_acro_form = true;
    let mut acro_form = dictionary! {
        "NeedAppearances" => false,
    };
    let mut acro_form_override = None;

    match case {
        "baseline" => {}
        "no_acroform" => include_acro_form = false,
        "need_appearances_absent" => {
            acro_form.remove(b"NeedAppearances");
        }
        "need_appearances_true" => acro_form.set("NeedAppearances", true),
        "xfa_present" => acro_form.set("XFA", Object::string_literal("xfa")),
        "need_appearances_false_indirect" => {
            acro_form.set(
                "NeedAppearances",
                document.add_object(Object::Boolean(false)),
            );
        }
        "need_appearances_true_indirect" => {
            acro_form.set(
                "NeedAppearances",
                document.add_object(Object::Boolean(true)),
            );
        }
        "need_appearances_wrong_type" => acro_form.set("NeedAppearances", 1),
        "need_appearances_null" => acro_form.set("NeedAppearances", Object::Null),
        "acroform_wrong_type" => acro_form_override = Some(42.into()),
        "acroform_stream_true" => {
            acro_form_override = Some(Object::Stream(Stream::new(
                dictionary! {"NeedAppearances" => true},
                Vec::new(),
            )));
        }
        "widget_missing_ap" => {
            widget.remove(b"AP");
        }
        "widget_indirect_subtype_missing_ap" => {
            let subtype = document.add_object(Object::Name(b"Widget".to_vec()));
            widget.set("Subtype", subtype);
            widget.remove(b"AP");
        }
        "stream_widget_missing_ap" => {
            widget.remove(b"AP");
            widget.remove(b"F");
            stream_widget = true;
            include_as_field = false;
        }
        "widget_empty_ap" => widget.set("AP", Dictionary::new()),
        "widget_wrong_type_ap" => widget.set("AP", 42),
        "widget_stream_ap" => widget.set("AP", appearance_stream),
        "widget_indirect_ap" => {
            let appearance = document.add_object(dictionary! {"N" => appearance_stream});
            widget.set("AP", appearance);
        }
        "non_widget_missing_ap" => {
            widget.set("Subtype", "Text");
            widget.remove(b"AP");
            include_as_field = false;
        }
        "field_only_widget_missing_ap" => {
            widget.remove(b"AP");
            include_on_page = false;
        }
        "direct_widget_missing_ap" => {
            widget.remove(b"AP");
            direct_widget = true;
            include_as_field = false;
        }
        "widget_parent_ap_only" => {
            let parent = document.add_object(dictionary! {
                "FT" => "Tx",
                "AP" => dictionary! {"N" => appearance_stream},
            });
            widget.remove(b"AP");
            widget.set("Parent", parent);
            include_as_field = false;
        }
        "unreferenced_widget_missing_ap" => {
            widget.remove(b"AP");
            include_on_page = false;
            include_as_field = false;
        }
        _ => panic!("unknown form fixture case {case}"),
    }

    let widget_value = if stream_widget {
        Object::Reference(document.add_object(Stream::new(widget, Vec::new())))
    } else if direct_widget {
        Object::Dictionary(widget)
    } else {
        Object::Reference(document.add_object(widget))
    };
    if include_as_field {
        acro_form.set("Fields", vec![widget_value.clone()]);
    } else {
        acro_form.set("Fields", Vec::<Object>::new());
    }

    let contents_id = document.add_object(Stream::new(Dictionary::new(), Vec::new()));
    let mut page = dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => Dictionary::new(),
        "Contents" => contents_id,
    };
    if include_on_page {
        page.set("Annots", vec![widget_value]);
    }
    let page_id = document.add_object(page);
    wrap_pages(&mut document, pages_id, page_id);

    let metadata_id = standard_metadata_stream(&mut document);
    let output_intents = single_profile_intent(
        &mut document,
        icc_header(*b"mntr", *b"RGB ", 2, 1),
        Some("GTS_PDFA1"),
    );
    let mut catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "OutputIntents" => output_intents.expect("output intent"),
    };
    if include_acro_form {
        let acro_form = acro_form_override.unwrap_or_else(|| Object::Dictionary(acro_form));
        catalog.set("AcroForm", document.add_object(acro_form));
    }
    let catalog_id = document.add_object(catalog);
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).expect("save form fixture");
    bytes
}

pub fn document_feature_fixture(case: &str) -> Vec<u8> {
    let mut document = pdf_document();
    let pages_id = document.new_object_id();
    let contents_id = document.add_object(Stream::new(Dictionary::new(), Vec::new()));
    let media_box = match case {
        "page_boundary_too_small" => vec![0.into(), 0.into(), 2.into(), 2.into()],
        "page_boundary_too_large" => vec![0.into(), 0.into(), 14_401.into(), 14_401.into()],
        _ => vec![0.into(), 0.into(), 612.into(), 792.into()],
    };
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => media_box,
        "Resources" => Dictionary::new(),
        "Contents" => contents_id,
    });
    wrap_pages(&mut document, pages_id, page_id);
    let metadata_id = standard_metadata_stream(&mut document);
    let output_intents = single_profile_intent(
        &mut document,
        icc_header(*b"mntr", *b"RGB ", 2, 1),
        Some("GTS_PDFA1"),
    );
    let mut catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "OutputIntents" => output_intents.expect("output intent"),
    };

    match case {
        "baseline" | "page_boundary_too_small" | "page_boundary_too_large" => {}
        "permissions_allowed" => catalog.set("Perms", dictionary! { "UR3" => Dictionary::new() }),
        "permissions_invalid" => catalog.set("Perms", dictionary! { "Unexpected" => true }),
        "embedded_file_invalid_pdfa" => {
            let embedded =
                document.add_object(Stream::new(Dictionary::new(), b"not a PDF".to_vec()));
            let file_spec = document.add_object(dictionary! {
                "Type" => "Filespec",
                "F" => Object::string_literal("file.bin"),
                "UF" => Object::string_literal("file.bin"),
                "EF" => dictionary! { "F" => embedded },
            });
            catalog.set(
                "Names",
                dictionary! {
                    "EmbeddedFiles" => dictionary! {
                        "Names" => vec![
                            Object::string_literal("file"),
                            Object::Reference(file_spec),
                        ],
                    },
                },
            );
        }
        "signature_reference_digest" => {
            let signature = document.add_object(dictionary! {
                "Type" => "Sig",
                "Filter" => "Adobe.PPKLite",
                "SubFilter" => "adbe.pkcs7.detached",
                "ByteRange" => vec![0.into(), 0.into(), 0.into(), 0.into()],
                "Contents" => Object::String(vec![0; 8], StringFormat::Hexadecimal),
                "Reference" => Object::Array(vec![Object::Dictionary(dictionary! {
                    "TransformMethod" => "DocMDP",
                    "DigestMethod" => "SHA256",
                    "TransformParams" => dictionary! {
                        "Type" => "TransformParams",
                        "P" => 1,
                        "V" => "1.2",
                    },
                })]),
            });
            catalog.set("Perms", dictionary! { "DocMDP" => signature });
            let field = document.add_object(dictionary! {
                "FT" => "Sig",
                "T" => Object::string_literal("Signature1"),
                "V" => signature,
            });
            catalog.set(
                "AcroForm",
                dictionary! { "Fields" => Object::Array(vec![Object::Reference(field)]) },
            );
        }
        "alternate_presentations" => {
            catalog.set(
                "Names",
                dictionary! { "AlternatePresentations" => Dictionary::new() },
            );
        }
        "catalog_requirements" => {
            catalog.set(
                "Requirements",
                Object::Array(vec![Object::Dictionary(dictionary! {
                    "Type" => "Requirement",
                    "S" => "EnableJavaScript"
                })]),
            );
        }
        "ocproperties_missing_name"
        | "ocproperties_duplicate_name"
        | "ocproperties_order_missing"
        | "ocproperties_as_present" => {
            let first = document.add_object(dictionary! {
                "Type" => "OCG",
                "Name" => Object::string_literal(if case == "ocproperties_duplicate_name" {
                    "Layer"
                } else {
                    "First"
                }),
            });
            let second = document.add_object(dictionary! {
                "Type" => "OCG",
                "Name" => Object::string_literal("Layer"),
            });
            let mut default = dictionary! {
                "Order" => if case == "ocproperties_order_missing" {
                    Vec::<Object>::new()
                } else {
                    vec![Object::Reference(first)]
                },
            };
            if case == "ocproperties_missing_name" {
                default.remove(b"Order");
            }
            if case == "ocproperties_as_present" {
                default.set("AS", Vec::<Object>::new());
            }
            if case == "ocproperties_duplicate_name" {
                default.set("Name", Object::string_literal("Layer"));
            }
            let mut properties = dictionary! {
                "OCGs" => vec![Object::Reference(first), Object::Reference(second)],
                "D" => default,
            };
            if case == "ocproperties_duplicate_name" {
                properties.set(
                    "Configs",
                    Object::Array(vec![
                        Object::Dictionary(dictionary! {
                            "Name" => Object::string_literal("Layer")
                        }),
                        Object::Dictionary(dictionary! {
                            "Name" => Object::string_literal("Layer")
                        }),
                    ]),
                );
            }
            catalog.set("OCProperties", properties);
        }
        "lang_catalog_valid" => catalog.set("Lang", Object::string_literal("en-US")),
        "lang_catalog_empty" => catalog.set("Lang", Object::string_literal("")),
        "lang_catalog_invalid" => catalog.set("Lang", Object::string_literal("en--US")),
        "lang_catalog_overlong" => catalog.set("Lang", Object::string_literal("abcdefghij")),
        "lang_catalog_wrong_type" => catalog.set("Lang", 1),
        "lang_catalog_null" => catalog.set("Lang", Object::Null),
        "lang_catalog_indirect_invalid" => {
            catalog.set("Lang", document.add_object(Object::string_literal("en_US")));
        }
        "lang_structure_valid" | "lang_structure_indirect_invalid" => {
            catalog.set("MarkInfo", dictionary! { "Marked" => true });
            let lang = if case == "lang_structure_indirect_invalid" {
                Object::Reference(document.add_object(Object::string_literal("fr--CA")))
            } else {
                Object::string_literal("fr-CA")
            };
            catalog.set(
                "StructTreeRoot",
                dictionary! {
                    "K" => dictionary! { "S" => "P", "Lang" => lang }
                },
            );
        }
        "lang_structure_invalid" => {
            catalog.set("MarkInfo", dictionary! { "Marked" => true });
            catalog.set(
                "StructTreeRoot",
                dictionary! {
                    "K" => dictionary! { "S" => "P", "Lang" => Object::string_literal("fr--CA") }
                },
            );
        }
        "lang_structure_wrong_type" => {
            catalog.set("MarkInfo", dictionary! { "Marked" => true });
            catalog.set(
                "StructTreeRoot",
                dictionary! {
                    "K" => dictionary! { "S" => "P", "Lang" => 1 }
                },
            );
        }
        "unicode_name_structure_invalid" => {
            catalog.set("MarkInfo", dictionary! { "Marked" => true });
            catalog.set(
                "StructTreeRoot",
                dictionary! {
                    "K" => dictionary! { "S" => Object::Name(b"P\xff".to_vec()) }
                },
            );
        }
        "lang_property_valid"
        | "lang_property_invalid"
        | "lang_property_indirect_invalid"
        | "lang_property_wrong_type"
        | "lang_property_null" => {
            catalog.set("MarkInfo", dictionary! { "Marked" => true });
            catalog.set("StructTreeRoot", Dictionary::new());
            let lang = match case {
                "lang_property_valid" => Object::string_literal("de-DE"),
                "lang_property_invalid" | "lang_property_indirect_invalid" => {
                    Object::string_literal("de--DE")
                }
                "lang_property_wrong_type" => Object::Integer(1),
                _ => Object::Null,
            };
            let properties = if case == "lang_property_indirect_invalid" {
                let properties = document.add_object(dictionary! { "Lang" => lang });
                let page = document.objects.get_mut(&page_id).expect("page");
                if let Object::Dictionary(page) = page {
                    page.set(
                        "Resources",
                        dictionary! {
                            "Properties" => dictionary! { "P1" => properties }
                        },
                    );
                }
                Object::Name(b"P1".to_vec())
            } else {
                Object::Dictionary(dictionary! { "Lang" => lang })
            };
            let stream = document.objects.get_mut(&contents_id).expect("contents");
            if let Object::Stream(stream) = stream {
                stream.content = content(vec![
                    Operation::new("BDC", vec![Object::Name(b"Span".to_vec()), properties]),
                    Operation::new("EMC", Vec::new()),
                ]);
                stream.dict.set("Length", stream.content.len() as i64);
            }
        }
        "tagged_valid" => {
            catalog.set("MarkInfo", dictionary! { "Marked" => true });
            catalog.set("StructTreeRoot", Dictionary::new());
        }
        "tagged_missing" => {}
        "tagged_false" => {
            catalog.set("MarkInfo", dictionary! { "Marked" => false });
        }
        "tagged_marked_wrong_type" => {
            catalog.set("MarkInfo", dictionary! { "Marked" => 1 });
        }
        "tagged_mark_info_wrong_type" => catalog.set("MarkInfo", 1),
        "tagged_indirect_mark_info_wrong_type" => {
            let mark_info = document.add_object(1);
            catalog.set("MarkInfo", mark_info);
        }
        "tagged_indirect_mark_info_null" => {
            let mark_info = document.add_object(Object::Null);
            catalog.set("MarkInfo", mark_info);
        }
        "tagged_indirect_mark_info" => {
            let mark_info = document.add_object(dictionary! { "Marked" => true });
            catalog.set("MarkInfo", mark_info);
        }
        "tagged_indirect_marked" => {
            let marked = document.add_object(true);
            catalog.set("MarkInfo", dictionary! { "Marked" => marked });
        }
        "tagged_struct_tree_only" => {
            catalog.set("StructTreeRoot", Dictionary::new());
        }
        "struct_tree_direct_valid" => {
            catalog.set("StructTreeRoot", Dictionary::new());
        }
        "struct_tree_minimal_valid" => {
            catalog.set("StructTreeRoot", Dictionary::new());
        }
        "struct_tree_missing" => {}
        "struct_tree_indirect_valid" => {
            let root = document.add_object(Dictionary::new());
            catalog.set("StructTreeRoot", root);
        }
        "struct_tree_invalid" => catalog.set("StructTreeRoot", 1),
        "struct_tree_indirect_invalid" => {
            let invalid = document.add_object(1);
            catalog.set("StructTreeRoot", invalid);
        }
        "struct_tree_unsupported_shape" => {
            catalog.set(
                "StructTreeRoot",
                dictionary! {
                    "K" => vec![Object::Dictionary(dictionary! { "Type" => "Unsupported" })]
                },
            );
        }
        "struct_tree_cyclic" => {
            let root = document.new_object_id();
            let child = document.new_object_id();
            document.objects.insert(
                root,
                Object::Dictionary(dictionary! { "K" => vec![Object::Reference(child)] }),
            );
            document.objects.insert(
                child,
                Object::Dictionary(dictionary! {
                    "S" => "P",
                    "P" => Object::Reference(root),
                    "K" => vec![Object::Reference(child)],
                }),
            );
            catalog.set("StructTreeRoot", root);
        }
        "struct_tree_parent_child" => {
            let root = document.new_object_id();
            let child = document.new_object_id();
            document.objects.insert(
                root,
                Object::Dictionary(dictionary! { "K" => vec![Object::Reference(child)] }),
            );
            document.objects.insert(
                child,
                Object::Dictionary(dictionary! {
                    "S" => "P",
                    "P" => Object::Reference(root),
                    "K" => Vec::<Object>::new(),
                }),
            );
            catalog.set("StructTreeRoot", root);
        }
        "struct_tree_role_map_self_cycle" => {
            catalog.set("MarkInfo", dictionary! { "Marked" => true });
            catalog.set(
                "StructTreeRoot",
                dictionary! {
                    "RoleMap" => dictionary! { "Custom" => "Custom" },
                    "K" => dictionary! { "S" => "Custom" },
                },
            );
        }
        "struct_tree_role_map_two_node_cycle" => {
            catalog.set("MarkInfo", dictionary! { "Marked" => true });
            catalog.set(
                "StructTreeRoot",
                dictionary! {
                    "RoleMap" => dictionary! {
                        "CustomA" => "CustomB",
                        "CustomB" => "CustomA",
                    },
                    "K" => dictionary! { "S" => "CustomA" },
                },
            );
        }
        "struct_tree_role_map_long_cycle" => {
            catalog.set("MarkInfo", dictionary! { "Marked" => true });
            let target = document.add_object(Object::Name(b"CustomB".to_vec()));
            let role_map = document.add_object(dictionary! {
                "CustomA" => target,
                "CustomB" => "CustomC",
                "CustomC" => "CustomA",
            });
            let root = document.add_object(dictionary! {
                "RoleMap" => role_map,
                "K" => dictionary! { "S" => "CustomA" },
            });
            catalog.set("StructTreeRoot", root);
        }
        "struct_tree_role_map_acyclic_chain" => {
            catalog.set("MarkInfo", dictionary! { "Marked" => true });
            let target = document.add_object(Object::Name(b"CustomB".to_vec()));
            let role_map = document.add_object(dictionary! {
                "CustomA" => target,
                "CustomB" => "P",
            });
            catalog.set(
                "StructTreeRoot",
                dictionary! {
                    "RoleMap" => role_map,
                    "K" => dictionary! { "S" => "CustomA" },
                },
            );
        }
        "struct_tree_role_map_unmapped" => {
            catalog.set("MarkInfo", dictionary! { "Marked" => true });
            catalog.set(
                "StructTreeRoot",
                dictionary! { "K" => dictionary! { "S" => "Custom" } },
            );
        }
        "struct_tree_role_map_direct" => {
            catalog.set("MarkInfo", dictionary! { "Marked" => true });
            catalog.set(
                "StructTreeRoot",
                dictionary! {
                    "RoleMap" => dictionary! { "Custom" => "P" },
                    "K" => dictionary! { "S" => "Custom" },
                },
            );
        }
        "struct_tree_role_map_standard_remap" => {
            catalog.set("MarkInfo", dictionary! { "Marked" => true });
            catalog.set(
                "StructTreeRoot",
                dictionary! {
                    "RoleMap" => dictionary! { "P" => "Custom" },
                    "K" => dictionary! { "S" => "P" },
                },
            );
        }
        "struct_tree_role_map_multi_step" => {
            catalog.set("MarkInfo", dictionary! { "Marked" => true });
            catalog.set(
                "StructTreeRoot",
                dictionary! {
                    "RoleMap" => dictionary! {
                        "CustomA" => "CustomB",
                        "CustomB" => "P",
                    },
                    "K" => dictionary! { "S" => "CustomA" },
                },
            );
        }
        "struct_tree_role_map_wrong_type" => {
            catalog.set("MarkInfo", dictionary! { "Marked" => true });
            catalog.set(
                "StructTreeRoot",
                dictionary! {
                    "RoleMap" => dictionary! { "Custom" => 1 },
                    "K" => dictionary! { "S" => "Custom" },
                },
            );
        }
        "struct_tree_role_map_invalid_target" => {
            catalog.set("MarkInfo", dictionary! { "Marked" => true });
            catalog.set(
                "StructTreeRoot",
                dictionary! {
                    "RoleMap" => dictionary! { "Custom" => "NotAStandardType" },
                    "K" => dictionary! { "S" => "Custom" },
                },
            );
        }
        "names_empty" => catalog.set("Names", Dictionary::new()),
        "names_embedded_files_dictionary" => {
            catalog.set("Names", dictionary! {"EmbeddedFiles" => Dictionary::new()});
        }
        "names_embedded_files_wrong_type" => {
            catalog.set("Names", dictionary! {"EmbeddedFiles" => 42});
        }
        "names_embedded_files_null" => {
            catalog.set("Names", dictionary! {"EmbeddedFiles" => Object::Null});
        }
        "names_embedded_files_indirect_null" => {
            let null = document.add_object(Object::Null);
            catalog.set("Names", dictionary! {"EmbeddedFiles" => null});
        }
        "names_stream_embedded_files" => {
            let names =
                document.add_object(Stream::new(dictionary! {"EmbeddedFiles" => 42}, Vec::new()));
            catalog.set("Names", names);
        }
        "names_wrong_type" => catalog.set("Names", 42),
        "names_indirect_dictionary" => {
            let names = document.add_object(dictionary! {"EmbeddedFiles" => 42});
            catalog.set("Names", names);
        }
        "unreferenced_names_embedded_files" => {
            document.add_object(dictionary! {"EmbeddedFiles" => 42});
        }
        "file_spec_without_ef" => {
            catalog.set(
                "Names",
                dictionary! {
                    "EmbeddedFiles" => dictionary! {
                        "Names" => vec![
                            Object::string_literal("file"),
                            Object::Dictionary(Dictionary::new()),
                        ],
                    },
                },
            );
        }
        "file_spec_with_ef" => {
            catalog.set(
                "Names",
                dictionary! {
                    "EmbeddedFiles" => dictionary! {
                        "Names" => vec![
                            Object::string_literal("file"),
                            Object::Dictionary(dictionary! {"EF" => Dictionary::new()}),
                        ],
                    },
                },
            );
        }
        "file_spec_missing_f_uf" | "embedded_file_missing_mime" => {
            let mut embedded_dictionary = Dictionary::new();
            if case == "embedded_file_missing_mime" {
                embedded_dictionary.set("Type", "EmbeddedFile");
            } else {
                embedded_dictionary.set("Subtype", "text/plain");
            }
            let embedded =
                document.add_object(Stream::new(embedded_dictionary, b"embedded data".to_vec()));
            let file_spec = document.add_object(dictionary! {
                "Type" => "Filespec",
                "EF" => dictionary! { "F" => embedded },
            });
            catalog.set(
                "Names",
                dictionary! {
                    "EmbeddedFiles" => dictionary! {
                        "Names" => vec![
                            Object::string_literal("file"),
                            Object::Reference(file_spec),
                        ],
                    },
                },
            );
        }
        "file_spec_direct_null_ef" => {
            catalog.set(
                "Names",
                dictionary! {
                    "EmbeddedFiles" => dictionary! {
                        "Names" => vec![
                            Object::string_literal("file"),
                            Object::Dictionary(dictionary! {"EF" => Object::Null}),
                        ],
                    },
                },
            );
        }
        "file_spec_indirect_null_ef" => {
            let null_ref = document.add_object(Object::Null);
            catalog.set(
                "Names",
                dictionary! {
                    "EmbeddedFiles" => dictionary! {
                        "Names" => vec![
                            Object::string_literal("file"),
                            Object::Dictionary(dictionary! {"EF" => null_ref}),
                        ],
                    },
                },
            );
        }
        "file_spec_indirect_with_ef" => {
            let file_spec = document.add_object(dictionary! {"EF" => Dictionary::new()});
            catalog.set(
                "Names",
                dictionary! {
                    "EmbeddedFiles" => dictionary! {
                        "Names" => vec![
                            Object::string_literal("file"),
                            Object::Reference(file_spec),
                        ],
                    },
                },
            );
        }
        "file_spec_stream_with_ef" => {
            let file_spec = document.add_object(Stream::new(
                dictionary! {"EF" => Dictionary::new()},
                Vec::new(),
            ));
            catalog.set(
                "Names",
                dictionary! {
                    "EmbeddedFiles" => dictionary! {
                        "Names" => vec![
                            Object::string_literal("file"),
                            Object::Reference(file_spec),
                        ],
                    },
                },
            );
        }
        "file_spec_scalar" => {
            catalog.set(
                "Names",
                dictionary! {
                    "EmbeddedFiles" => dictionary! {
                        "Names" => vec![Object::string_literal("file"), Object::Integer(42)],
                    },
                },
            );
        }
        "embedded_files_kids_with_ef" => {
            let child = document.add_object(dictionary! {
                "Names" => vec![
                    Object::string_literal("file"),
                    Object::Dictionary(dictionary! {"EF" => Dictionary::new()}),
                ],
            });
            catalog.set(
                "Names",
                dictionary! {
                    "EmbeddedFiles" => dictionary! {
                        "Kids" => vec![Object::Reference(child)],
                    },
                },
            );
        }
        "stream_f" => {
            document.add_object(Stream::new(dictionary! {"F" => "external"}, Vec::new()));
        }
        "stream_ffilter" => {
            document.add_object(Stream::new(
                dictionary! {"FFilter" => "FlateDecode"},
                Vec::new(),
            ));
        }
        "stream_fdecodeparms" => {
            document.add_object(Stream::new(dictionary! {"FDecodeParms" => 42}, Vec::new()));
        }
        "stream_external_null" => {
            document.add_object(Stream::new(dictionary! {"F" => Object::Null}, Vec::new()));
        }
        "stream_lzwdecode" => {
            document.add_object(Stream::new(
                dictionary! {"Filter" => "LZWDecode"},
                Vec::new(),
            ));
        }
        "stream_lzwdecode_array" => {
            document.add_object(Stream::new(
                dictionary! {"Filter" => vec!["FlateDecode".into(), "LZWDecode".into()]},
                Vec::new(),
            ));
        }
        "stream_lzwdecode_indirect" => {
            let filter = document.add_object("LZWDecode");
            document.add_object(Stream::new(dictionary! {"Filter" => filter}, Vec::new()));
        }
        "stream_lzw_short_name" => {
            document.add_object(Stream::new(dictionary! {"Filter" => "LZW"}, Vec::new()));
        }
        "object_limits_at_boundary" => {
            document.add_object(Object::Integer(2_147_483_647));
            document.add_object(Object::Integer(-2_147_483_648));
            document.add_object(Object::Real(32_766.5));
            document.add_object(Object::Real(-32_766.5));
            document.add_object(Object::String(vec![b'x'; 65_535], StringFormat::Literal));
            document.add_object(Object::Name(vec![b'n'; 127]));
            document.add_object(Object::Array(vec![Object::Null; 8_191]));
            let mut dictionary = Dictionary::new();
            for index in 0..4_095 {
                dictionary.set(format!("K{index}"), Object::Null);
            }
            document.add_object(dictionary);
        }
        "object_integer_high" => {
            document.add_object(Object::Integer(2_147_483_648));
        }
        "object_integer_low" => {
            document.add_object(Object::Integer(-2_147_483_649));
        }
        "object_real_high" => {
            document.add_object(Object::Real(32_767.5));
        }
        "object_real_pdfa2_high" => {
            document.add_object(Object::Real(f32::MAX));
        }
        "object_real_low" => {
            document.add_object(Object::Real(-32_767.5));
        }
        "object_real_pdfa2_low" => {
            document.add_object(Object::Real(-f32::MAX));
        }
        "object_real_pdfa2_minimum" => {
            document.add_object(Object::Real(f32::MIN_POSITIVE / 2.0));
        }
        "object_string_long" => {
            document.add_object(Object::String(vec![b'x'; 65_536], StringFormat::Literal));
        }
        "object_name_long" => {
            document.add_object(Object::Name(vec![b'n'; 128]));
        }
        "object_dictionary_key_long" => {
            let mut dictionary = Dictionary::new();
            dictionary.set(vec![b'k'; 128], Object::Null);
            document.add_object(dictionary);
        }
        "object_array_long" => {
            document.add_object(Object::Array(vec![Object::Null; 8_192]));
        }
        "object_dictionary_long" => {
            let mut dictionary = Dictionary::new();
            for index in 0..4_096 {
                dictionary.set(format!("K{index}"), Object::Integer(1));
            }
            catalog.set("ObjectLimitProbe", dictionary);
        }
        "object_dictionary_long_nulls" => {
            let mut dictionary = Dictionary::new();
            for index in 0..4_096 {
                dictionary.set(format!("K{index}"), Object::Null);
            }
            catalog.set("ObjectLimitProbe", dictionary);
        }
        "ocproperties_dictionary" => catalog.set("OCProperties", Dictionary::new()),
        "ocproperties_wrong_type" => catalog.set("OCProperties", 42),
        "ocproperties_null" => catalog.set("OCProperties", Object::Null),
        "ocproperties_indirect_null" => {
            let null = document.add_object(Object::Null);
            catalog.set("OCProperties", null);
        }
        "ocproperties_stream" => {
            let value = document.add_object(Stream::new(Dictionary::new(), Vec::new()));
            catalog.set("OCProperties", value);
        }
        "unreferenced_catalog_ocproperties" => {
            document.add_object(dictionary! {
                "Type" => "Catalog",
                "OCProperties" => Dictionary::new(),
            });
        }
        _ => panic!("unknown document-feature fixture case {case}"),
    }

    let catalog_id = document.add_object(catalog);
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save document-feature fixture");
    bytes
}

pub fn tagged_document_fixture(case: &str) -> Vec<u8> {
    let mut bytes = document_feature_fixture(case);
    let from = b"pdfaid:conformance=\"B\"";
    let at = bytes
        .windows(from.len())
        .position(|window| window == from)
        .expect("PDF/A-1b conformance declaration");
    *bytes
        .get_mut(at + from.len() - 2)
        .expect("tagged PDF/A conformance marker") = b'A';
    bytes
}

/// The object-limit cases (`object_integer_high`, `object_array_long`, ...)
/// are generated as extra `document_feature_fixture` match arms rather than
/// their own builder, since both share the same minimal catalog/page
/// scaffolding. This alias just names the fixture for its own test file.
pub fn object_limit_fixture(case: &str) -> Vec<u8> {
    let mut bytes = document_feature_fixture(case);
    if case == "object_real_pdfa2_high" {
        let source = b"340282350000000000000000000000000000000";
        let replacement = b"340400000000000000000000000000000000000.0";
        let start = bytes
            .windows(source.len())
            .position(|window| window == source)
            .expect("serialized PDF/A-2 high real");
        let old_xref = find_last_startxref(&bytes);
        bytes.splice(start..start + source.len(), replacement.iter().copied());
        let delta = replacement.len() as isize - source.len() as isize;
        let new_xref = usize::try_from(old_xref as isize + delta).expect("shifted xref offset");
        let xref_end = bytes
            .get(new_xref..)
            .expect("xref range")
            .windows(b"trailer\n".len())
            .position(|window| window == b"trailer\n")
            .map(|offset| new_xref + offset)
            .expect("xref trailer");
        let mut line_start = new_xref + b"xref\n".len();
        while line_start < xref_end {
            let line_end = bytes
                .get(line_start..xref_end)
                .expect("xref line range")
                .iter()
                .position(|byte| *byte == b'\n')
                .map(|offset| line_start + offset)
                .expect("xref line ending");
            if line_end - line_start >= 18
                && bytes
                    .get(line_start..line_start + 10)
                    .expect("xref offset field")
                    .iter()
                    .all(u8::is_ascii_digit)
                && *bytes.get(line_start + 17).expect("xref entry marker") == b'n'
            {
                let old_offset = std::str::from_utf8(
                    bytes
                        .get(line_start..line_start + 10)
                        .expect("xref offset field"),
                )
                .expect("xref offset")
                .parse::<usize>()
                .expect("numeric xref offset");
                let shifted = if old_offset > start {
                    usize::try_from(old_offset as isize + delta).expect("shifted object offset")
                } else {
                    old_offset
                };
                let formatted = format!("{shifted:010}");
                bytes
                    .get_mut(line_start..line_start + 10)
                    .expect("xref offset field")
                    .copy_from_slice(formatted.as_bytes());
            }
            line_start = line_end + 1;
        }
        let startxref = bytes
            .get(xref_end..)
            .expect("startxref range")
            .windows(b"startxref\n".len())
            .position(|window| window == b"startxref\n")
            .map(|offset| xref_end + offset + b"startxref\n".len())
            .expect("startxref value");
        let startxref_end = bytes
            .get(startxref..)
            .expect("startxref value range")
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|offset| startxref + offset)
            .expect("startxref line ending");
        let formatted = new_xref.to_string();
        assert_eq!(formatted.len(), startxref_end - startxref);
        bytes
            .get_mut(startxref..startxref_end)
            .expect("startxref value field")
            .copy_from_slice(formatted.as_bytes());
    }
    bytes
}

/// A deliberately small classic-xref PDF used to exercise source syntax that
/// `lopdf` normally discards. The semantic baseline is intentionally sparse;
/// atomic tests compare only rule-ID deltas against the same baseline.
pub fn syntax_fixture(case: &str) -> Vec<u8> {
    if case.starts_with("stream_") {
        return syntax_stream_fixture(case);
    }
    let (initial_probe, current_probe, incremental) = match case {
        "baseline" => ("null".to_owned(), None, false),
        "duplicate_last_null" => ("<< /K 2147483648 /K null >>".to_owned(), None, false),
        "duplicate_last_invalid" => ("<< /K null /K 2147483648 >>".to_owned(), None, false),
        "escaped_name_at_boundary" => (format!("/{}", "A#41".repeat(63) + "A"), None, false),
        "escaped_name_over_boundary" => (format!("/{}", "A#41".repeat(64)), None, false),
        "literal_string_at_boundary" => (format!("({})", "\\101".repeat(65_535)), None, false),
        "literal_string_over_boundary" => (format!("({})", "\\101".repeat(65_536)), None, false),
        "hex_string_invalid_character" => ("<GG>".to_owned(), None, false),
        "hex_string_odd" => ("<ABC>".to_owned(), None, false),
        "incremental_stale_invalid" => ("2147483648".to_owned(), Some("1".to_owned()), true),
        "incremental_active_invalid" => ("1".to_owned(), Some("2147483648".to_owned()), true),
        "empty_trailer_id" => ("null".to_owned(), None, false),
        "single_trailer_id" => ("null".to_owned(), None, false),
        "wrong_type_trailer_id" => ("null".to_owned(), None, false),
        _ => panic!("unknown syntax fixture case {case}"),
    };

    let trailer_id = match case {
        "empty_trailer_id" => "[]",
        "single_trailer_id" => "[(one)]",
        "wrong_type_trailer_id" => "(not-an-array)",
        _ => "[(one) (two)]",
    };
    build_classic_pdf(
        &initial_probe,
        current_probe.as_deref(),
        incremental,
        trailer_id,
    )
}

pub fn syntax_stream_fixture(case: &str) -> Vec<u8> {
    let (length_object, stream_dictionary) = match case {
        "stream_direct_length_valid" => (None, "<< /Length 3 >>"),
        "stream_direct_length_mismatch" => (None, "<< /Length 4 >>"),
        "stream_indirect_length_valid" => (Some("3"), "<< /Length 4 0 R >>"),
        "stream_indirect_length_mismatch" => (Some("4"), "<< /Length 4 0 R >>"),
        "stream_duplicate_length_last_valid" => (None, "<< /Length 4 /Length 3 >>"),
        "stream_duplicate_length_last_invalid" => (None, "<< /Length 3 /Length 4 >>"),
        "stream_duplicate_external_last_null" => (None, "<< /Length 3 /F (external) /F null >>"),
        "stream_duplicate_external_last_invalid" => (None, "<< /Length 3 /F null /F (external) >>"),
        "stream_escaped_lzw_filter" => (None, "<< /Length 3 /Filter /LZW#44ecode >>"),
        _ => panic!("unknown stream syntax fixture case {case}"),
    };
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>".to_owned(),
    ];
    if let Some(length) = length_object {
        objects.push(length.to_owned());
    } else {
        objects.push("null".to_owned());
    }
    objects.push(format!("{stream_dictionary}\nstream\nabc\nendstream"));
    build_classic_pdf_objects(&objects, "[(one) (two)]")
}

pub fn build_classic_pdf(
    initial_probe: &str,
    current_probe: Option<&str>,
    incremental: bool,
    trailer_id: &str,
) -> Vec<u8> {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>".to_owned(),
        initial_probe.to_owned(),
    ];
    let mut bytes = build_classic_pdf_objects(&objects, trailer_id);
    let first_xref = find_last_startxref(&bytes);

    if incremental {
        let object = current_probe.expect("incremental object");
        let replacement_offset = bytes.len();
        write!(bytes, "4 0 obj\n{object}\nendobj\n").expect("write replacement");
        let second_xref = bytes.len();
        write!(
            bytes,
            "xref\n4 1\n{replacement_offset:010} 00000 n \n\
             trailer\n<< /Size 5 /Root 1 0 R /Prev {first_xref} /ID {trailer_id} >>\n\
             startxref\n{second_xref}\n%%EOF\n"
        )
        .expect("write incremental trailer");
    }
    bytes
}

pub fn build_classic_pdf_objects(objects: &[String], trailer_id: &str) -> Vec<u8> {
    let mut bytes = b"%PDF-1.4\n%\x80\x81\x82\x83\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        write!(bytes, "{} 0 obj\n{}\nendobj\n", index + 1, object).expect("write object");
    }
    let first_xref = bytes.len();
    write!(bytes, "xref\n0 {}\n", objects.len() + 1).expect("write xref");
    bytes.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
        writeln!(bytes, "{offset:010} 00000 n ").expect("write xref entry");
    }
    write!(
        bytes,
        "trailer\n<< /Size {} /Root 1 0 R /ID {} >>\nstartxref\n{}\n%%EOF\n",
        objects.len() + 1,
        trailer_id,
        first_xref
    )
    .expect("write trailer");
    bytes
}

pub fn find_last_startxref(bytes: &[u8]) -> usize {
    let marker = bytes
        .windows(b"startxref\n".len())
        .rposition(|window| window == b"startxref\n")
        .expect("startxref")
        + b"startxref\n".len();
    let end = bytes
        .get(marker..)
        .expect("startxref range")
        .iter()
        .position(|byte| !byte.is_ascii_digit())
        .map(|length| marker + length)
        .expect("startxref end");
    std::str::from_utf8(bytes.get(marker..end).expect("startxref field"))
        .expect("startxref UTF-8")
        .parse()
        .expect("startxref integer")
}

pub fn action(subtype: &str) -> Object {
    Object::Dictionary(action_dictionary(subtype))
}

pub fn action_dictionary(subtype: &str) -> Dictionary {
    dictionary! {
        "Type" => "Action",
        "S" => subtype,
    }
}

pub fn file_spec_with_ef(target: &str) -> Dictionary {
    dictionary! {
        "Type" => "Filespec",
        "F" => Object::string_literal(target),
        "EF" => dictionary! {"F" => Dictionary::new()},
    }
}

pub fn valid_annotation(subtype: &str) -> Dictionary {
    dictionary! {
        "Type" => "Annot",
        "Subtype" => subtype,
        "Rect" => vec![0.into(), 0.into(), 10.into(), 10.into()],
        "F" => 4,
    }
}

pub fn canonical_form_fixture() -> Vec<u8> {
    let mut document = Document::load_mem(include_bytes!("../fixtures/canonical-pdfa-1a.pdf"))
        .expect("load canonical PDF/A-1a fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("canonical page");
    let appearance = document.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 120.into(), 24.into()],
        },
        b"q 0.85 0.92 1 rg 0 0 120 24 re f Q".to_vec(),
    ));
    let widget = document.add_object(dictionary! {
        "Type" => "Annot",
        "Subtype" => "Widget",
        "Rect" => vec![0.into(), 0.into(), 120.into(), 24.into()],
        "F" => 4,
        "FT" => "Tx",
        "T" => Object::string_literal("canonical-field"),
        "AP" => dictionary! {"N" => appearance},
    });
    document
        .get_object_mut(page_id)
        .expect("canonical page object")
        .as_dict_mut()
        .expect("canonical page dictionary")
        .set("Annots", vec![Object::Reference(widget)]);
    let acro_form = document.add_object(dictionary! {
        "NeedAppearances" => false,
        "Fields" => vec![Object::Reference(widget)],
    });
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("canonical root")
        .as_reference()
        .expect("indirect canonical root");
    document
        .get_object_mut(root_id)
        .expect("canonical catalog object")
        .as_dict_mut()
        .expect("canonical catalog dictionary")
        .set("AcroForm", acro_form);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save canonical form fixture");
    bytes
}

pub fn canonical_a1a_unused_invalid_font_fixture() -> Vec<u8> {
    let mut document = Document::load_mem(include_bytes!("../fixtures/canonical-pdfa-1a.pdf"))
        .expect("load canonical PDF/A-1a fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("canonical page");
    let resource_ids = document
        .get_page_resources(page_id)
        .expect("canonical page resources")
        .1;
    let resources_id = *resource_ids.first().expect("indirect canonical resources");
    let font_resources = document
        .get_object(resources_id)
        .expect("canonical resources object")
        .as_dict()
        .expect("canonical resources dictionary")
        .get(b"Font")
        .expect("canonical font resources")
        .clone();
    let invalid_font = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "UnsupportedFont",
        "BaseFont" => "UnusedUnsupportedFont",
    });
    match font_resources {
        Object::Reference(font_id) => {
            document
                .get_object_mut(font_id)
                .expect("canonical font resource dictionary")
                .as_dict_mut()
                .expect("canonical font resource dictionary")
                .set("Unused", invalid_font);
        }
        Object::Dictionary(mut fonts) => {
            fonts.set("Unused", invalid_font);
            document
                .get_object_mut(resources_id)
                .expect("canonical resources object")
                .as_dict_mut()
                .expect("canonical resources dictionary")
                .set("Font", fonts);
        }
        _ => panic!("canonical font resources have an unsupported shape"),
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save unused invalid-font fixture");
    bytes
}

pub fn canonical_a1b_truetype_glyph_fixture(missing_glyph: bool) -> Vec<u8> {
    let mut document = Document::load_mem(include_bytes!("../fixtures/canonical-pdfa-1b.pdf"))
        .expect("load canonical PDF/A-1b fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("canonical page");
    let mut descriptor_dictionary = font_descriptor(&mut document, true);
    descriptor_dictionary.set(
        "FontFile2",
        document.add_object(Stream::new(
            Dictionary::new(),
            sfnt::minimal_truetype_with_cmap_count_and_mapping_and_glyph_count(
                1,
                33,
                if missing_glyph { 1 } else { 2 },
            ),
        )),
    );
    let descriptor = document.add_object(descriptor_dictionary);
    let font = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "TrueType",
        "BaseFont" => "MaiTestFont",
        "Encoding" => "WinAnsiEncoding",
        "FirstChar" => 33,
        "LastChar" => 33,
        "Widths" => vec![500.into()],
        "FontDescriptor" => descriptor,
    });
    let page = document
        .get_object(page_id)
        .expect("canonical page object")
        .as_dict()
        .expect("canonical page dictionary")
        .clone();
    let resources = page
        .get(b"Resources")
        .expect("canonical page resources")
        .clone();
    let resource_dictionary = match resources {
        Object::Reference(resources_id) => document
            .get_object_mut(resources_id)
            .expect("canonical resources object")
            .as_dict_mut()
            .expect("canonical resources dictionary"),
        Object::Dictionary(resources) => {
            document
                .get_object_mut(page_id)
                .expect("canonical page object")
                .as_dict_mut()
                .expect("canonical page dictionary")
                .set("Resources", resources);
            document
                .get_object_mut(page_id)
                .expect("canonical page object")
                .as_dict_mut()
                .expect("canonical page dictionary")
                .get_mut(b"Resources")
                .expect("canonical page resources")
                .as_dict_mut()
                .expect("canonical resources dictionary")
        }
        _ => panic!("canonical resources have an unsupported shape"),
    };
    let fonts = resource_dictionary
        .get(b"Font")
        .ok()
        .cloned()
        .unwrap_or_else(|| Object::Dictionary(Dictionary::new()));
    match fonts {
        Object::Reference(fonts_id) => document
            .get_object_mut(fonts_id)
            .expect("canonical font resources")
            .as_dict_mut()
            .expect("canonical font resource dictionary")
            .set("FExtra", font),
        Object::Dictionary(mut fonts) => {
            fonts.set("FExtra", font);
            resource_dictionary.set("Font", fonts);
        }
        _ => panic!("canonical font resources have an unsupported shape"),
    }
    let extra_content = b"BT /FExtra 12 Tf (!) Tj ET".to_vec();
    let extra_content_id = document.add_object(Stream::new(Dictionary::new(), extra_content));
    let contents = page
        .get(b"Contents")
        .expect("canonical page contents")
        .clone();
    document
        .get_object_mut(page_id)
        .expect("canonical page object")
        .as_dict_mut()
        .expect("canonical page dictionary")
        .set(
            "Contents",
            vec![contents, Object::Reference(extra_content_id)],
        );
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save canonical TrueType glyph fixture");
    bytes
}

pub fn canonical_a1b_header_offset_fixture() -> Vec<u8> {
    let mut bytes = b"%\n".to_vec();
    bytes.extend_from_slice(include_bytes!("../fixtures/canonical-pdfa-1b.pdf"));
    bytes
}

pub fn pdfa_2_3_fixture(case: &str) -> Vec<u8> {
    let mut document = pdf_document();
    let pages_id = document.new_object_id();
    let mut resources = Dictionary::new();
    let mut contents = Vec::new();
    let mut page = dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    };

    match case {
        "catalog_needs_rendering"
        | "file_spec_af_relationship"
        | "file_spec_association"
        | "pres_steps"
        | "signature_byte_range"
        | "signature_certificate"
        | "signature_signer_count" => {}
        "devicen_colorants" => {
            let tint_transform = dictionary! {
                "FunctionType" => 2,
                "Domain" => vec![0.into(), 1.into()],
                "C0" => vec![0.into(), 0.into(), 0.into()],
                "C1" => vec![1.into(), 1.into(), 1.into()],
                "N" => 1,
            };
            resources.set(
                "ColorSpace",
                dictionary! {
                    "CS1" => Object::Array(vec![
                        Object::Name(b"DeviceN".to_vec()),
                        Object::Array(vec![Object::Name(b"Spot".to_vec())]),
                        Object::Name(b"DeviceRGB".to_vec()),
                        Object::Dictionary(tint_transform),
                        Object::Dictionary(dictionary! { "Colorants" => Dictionary::new() }),
                    ]),
                },
            );
            contents = b"/CS1 cs\n0 0 0 scn\n".to_vec();
        }
        case if case.starts_with("jpeg2000_") => {
            let image_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 8,
                    "Filter" => "JPXDecode",
                },
                jpeg2000_bytes(case),
            ));
            resources.set("XObject", dictionary! { "Im" => image_id });
            contents = b"/Im Do\n".to_vec();
        }
        other => panic!("unknown PDF/A-2/3 fixture case {other}"),
    }

    if case == "pres_steps" {
        page.set("PresSteps", Dictionary::new());
    }
    let contents_id = document.add_object(Stream::new(Dictionary::new(), contents));
    page.set("Resources", resources);
    page.set("Contents", contents_id);
    let page_id = document.add_object(page);
    wrap_pages(&mut document, pages_id, page_id);

    let metadata_id = standard_metadata_stream(&mut document);
    let output_intents = single_profile_intent(
        &mut document,
        icc_header(*b"mntr", *b"RGB ", 2, 1),
        Some("GTS_PDFA1"),
    );
    let mut catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "OutputIntents" => output_intents.expect("output intent"),
    };

    match case {
        "catalog_needs_rendering" => catalog.set("NeedsRendering", true),
        "file_spec_af_relationship" | "file_spec_association" => {
            let embedded = document.add_object(Stream::new(
                dictionary! { "Subtype" => "application/pdf" },
                b"%PDF-1.4\n%%EOF\n".to_vec(),
            ));
            let mut file_spec = dictionary! {
                "Type" => "Filespec",
                "F" => Object::string_literal("attachment.pdf"),
                "UF" => Object::string_literal("attachment.pdf"),
                "EF" => dictionary! { "F" => embedded },
            };
            if case == "file_spec_association" {
                file_spec.set("AFRelationship", "Data");
            }
            let file_spec_id = document.add_object(file_spec);
            catalog.set(
                "Names",
                dictionary! {
                    "EmbeddedFiles" => dictionary! {
                        "Names" => vec![
                            Object::string_literal("attachment"),
                            Object::Reference(file_spec_id),
                        ],
                    },
                },
            );
            if case == "file_spec_af_relationship" {
                catalog.set("AF", vec![Object::Reference(file_spec_id)]);
            }
        }
        "signature_byte_range" | "signature_certificate" | "signature_signer_count" => {
            let (certificate, signer_count) = match case {
                "signature_certificate" => (false, 1),
                "signature_signer_count" => (true, 2),
                _ => (false, 0),
            };
            let contents = if case == "signature_byte_range" {
                vec![0; 8]
            } else {
                signature_pkcs7(certificate, signer_count)
            };
            let signature_id = document.add_object(dictionary! {
                "Type" => "Sig",
                "Filter" => "Adobe.PPKLite",
                "SubFilter" => "adbe.pkcs7.detached",
                "ByteRange" => vec![0.into(), 0.into(), 0.into(), 0.into()],
                "Contents" => Object::String(contents, StringFormat::Hexadecimal),
            });
            catalog.set("Perms", dictionary! { "DocMDP" => signature_id });
            let field = document.add_object(dictionary! {
                "FT" => "Sig",
                "T" => Object::string_literal("Signature1"),
                "V" => signature_id,
            });
            catalog.set(
                "AcroForm",
                dictionary! { "Fields" => vec![Object::Reference(field)] },
            );
        }
        _ => {}
    }

    let catalog_id = document.add_object(catalog);
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save PDF/A-2/3 fixture");
    bytes
}

fn jpeg2000_bytes(case: &str) -> Vec<u8> {
    let mut bytes = BASE_JPEG2000.to_vec();
    let ihdr = bytes
        .windows(4)
        .position(|window| window == b"ihdr")
        .expect("JPEG2000 ihdr box")
        + 4;
    let colr = bytes
        .windows(4)
        .position(|window| window == b"colr")
        .expect("JPEG2000 colr box")
        + 4;
    match case {
        "jpeg2000_bit_depth" => {
            *bytes.get_mut(ihdr + 10).expect("JPEG2000 bit-depth field") = 0x7f;
        }
        "jpeg2000_channels" => bytes
            .get_mut(ihdr + 8..ihdr + 10)
            .expect("JPEG2000 channel field")
            .copy_from_slice(&2u16.to_be_bytes()),
        "jpeg2000_color_method" => {
            *bytes.get_mut(colr).expect("JPEG2000 color-method field") = 4;
        }
        "jpeg2000_color_space" => bytes
            .get_mut(colr + 3..colr + 7)
            .expect("JPEG2000 color-space field")
            .copy_from_slice(&19u32.to_be_bytes()),
        "jpeg2000_color_specs" => {
            let jp2h = bytes
                .windows(4)
                .position(|window| window == b"jp2h")
                .expect("JPEG2000 jp2h box");
            let jp2c = bytes
                .windows(4)
                .position(|window| window == b"jp2c")
                .expect("JPEG2000 jp2c box")
                - 4;
            let length = u32::from_be_bytes(
                bytes
                    .get(jp2h - 4..jp2h)
                    .expect("JPEG2000 box length")
                    .try_into()
                    .expect("four-byte JPEG2000 box length"),
            ) + 15;
            bytes
                .get_mut(jp2h - 4..jp2h)
                .expect("JPEG2000 box length")
                .copy_from_slice(&length.to_be_bytes());
            bytes.splice(
                jp2c..jp2c,
                [0, 0, 0, 15, b'c', b'o', b'l', b'r', 1, 0, 0, 0, 0, 0, 0],
            );
        }
        other => panic!("unknown JPEG2000 fixture case {other}"),
    }
    bytes
}

fn signature_pkcs7(certificate: bool, signer_count: usize) -> Vec<u8> {
    const SIGNED_DATA_OID: &[u8] = &[
        0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x02,
    ];
    let mut fields = Vec::new();
    fields.extend(der_tlv(0x02, &[1]));
    fields.extend(der_tlv(0x31, &[]));
    fields.extend(der_tlv(0x30, &[]));
    if certificate {
        fields.extend(der_tlv(0xa0, &[]));
    }
    let mut signers = Vec::new();
    for _ in 0..signer_count {
        signers.extend(der_tlv(0x30, &[]));
    }
    fields.extend(der_tlv(0x31, &signers));
    let signed_data = der_tlv(0x30, &fields);
    let wrapper = der_tlv(0xa0, &signed_data);
    let mut content_info = Vec::new();
    content_info.extend_from_slice(SIGNED_DATA_OID);
    content_info.extend_from_slice(&wrapper);
    der_tlv(0x30, &content_info)
}

fn der_tlv(tag: u8, content: &[u8]) -> Vec<u8> {
    assert!(
        content.len() < 128,
        "test DER helper only supports short lengths"
    );
    let mut bytes = vec![tag, content.len() as u8];
    bytes.extend_from_slice(content);
    bytes
}

const BASE_JPEG2000: &[u8] = &[
    0x00, 0x00, 0x00, 0x0c, 0x6a, 0x50, 0x20, 0x20, 0x0d, 0x0a, 0x87, 0x0a, 0x00, 0x00, 0x00, 0x14,
    0x66, 0x74, 0x79, 0x70, 0x6a, 0x70, 0x32, 0x20, 0x00, 0x00, 0x00, 0x00, 0x6a, 0x70, 0x32, 0x20,
    0x00, 0x00, 0x00, 0x2d, 0x6a, 0x70, 0x32, 0x68, 0x00, 0x00, 0x00, 0x16, 0x69, 0x68, 0x64, 0x72,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x07, 0x07, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x0f, 0x63, 0x6f, 0x6c, 0x72, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x11, 0x00, 0x00, 0x00,
    0x84, 0x6a, 0x70, 0x32, 0x63, 0xff, 0x4f, 0xff, 0x51, 0x00, 0x29, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
    0x07, 0x01, 0x01, 0xff, 0x52, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x04, 0x04, 0x00,
    0x01, 0xff, 0x5c, 0x00, 0x04, 0x40, 0x40, 0xff, 0x64, 0x00, 0x25, 0x00, 0x01, 0x43, 0x72, 0x65,
    0x61, 0x74, 0x65, 0x64, 0x20, 0x62, 0x79, 0x20, 0x4f, 0x70, 0x65, 0x6e, 0x4a, 0x50, 0x45, 0x47,
    0x20, 0x76, 0x65, 0x72, 0x73, 0x69, 0x6f, 0x6e, 0x20, 0x32, 0x2e, 0x35, 0x2e, 0x34, 0xff, 0x90,
    0x00, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x12, 0x00, 0x01, 0xff, 0x93, 0xdf, 0x80, 0x08, 0x07,
    0xff, 0xd9,
];

pub fn canonical_a1b_blend_mode_fixture() -> Vec<u8> {
    let mut document = Document::load_mem(include_bytes!("../fixtures/canonical-pdfa-1b.pdf"))
        .expect("load canonical PDF/A-1b fixture");
    let page_id = *document
        .get_pages()
        .values()
        .next()
        .expect("canonical page");
    let page = document
        .get_object(page_id)
        .expect("canonical page object")
        .as_dict()
        .expect("canonical page dictionary")
        .clone();
    let resources = page
        .get(b"Resources")
        .expect("canonical page resources")
        .clone();
    let blend_state = document.add_object(dictionary! { "BM" => "Multiply" });
    let resource_dictionary = match resources {
        Object::Reference(resources_id) => document
            .get_object_mut(resources_id)
            .expect("canonical resources object")
            .as_dict_mut()
            .expect("canonical resources dictionary"),
        Object::Dictionary(resources) => {
            document
                .get_object_mut(page_id)
                .expect("canonical page object")
                .as_dict_mut()
                .expect("canonical page dictionary")
                .set("Resources", resources);
            document
                .get_object_mut(page_id)
                .expect("canonical page object")
                .as_dict_mut()
                .expect("canonical page dictionary")
                .get_mut(b"Resources")
                .expect("canonical page resources")
                .as_dict_mut()
                .expect("canonical resources dictionary")
        }
        _ => panic!("canonical resources have an unsupported shape"),
    };
    let extgstates = resource_dictionary
        .get(b"ExtGState")
        .ok()
        .cloned()
        .unwrap_or_else(|| Object::Dictionary(Dictionary::new()));
    match extgstates {
        Object::Reference(extgstates_id) => document
            .get_object_mut(extgstates_id)
            .expect("canonical ExtGState resources")
            .as_dict_mut()
            .expect("canonical ExtGState dictionary")
            .set("GSBlend", blend_state),
        Object::Dictionary(mut extgstates) => {
            extgstates.set("GSBlend", blend_state);
            resource_dictionary.set("ExtGState", extgstates);
        }
        _ => panic!("canonical ExtGState resources have an unsupported shape"),
    }
    let extra_content_id =
        document.add_object(Stream::new(Dictionary::new(), b"q /GSBlend gs Q".to_vec()));
    let contents = page
        .get(b"Contents")
        .expect("canonical page contents")
        .clone();
    document
        .get_object_mut(page_id)
        .expect("canonical page object")
        .as_dict_mut()
        .expect("canonical page dictionary")
        .set(
            "Contents",
            vec![contents, Object::Reference(extra_content_id)],
        );
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save canonical blend-mode fixture");
    bytes
}

pub fn canonical_a1a_mutation(case: &str) -> Vec<u8> {
    let mut document = Document::load_mem(include_bytes!("../fixtures/canonical-pdfa-1a.pdf"))
        .expect("load canonical PDF/A-1a fixture");
    let root_id = document
        .trailer
        .get(b"Root")
        .expect("canonical root")
        .as_reference()
        .expect("indirect canonical root");
    let catalog = document
        .get_object(root_id)
        .expect("canonical catalog")
        .as_dict()
        .expect("canonical catalog dictionary")
        .clone();

    match case {
        "id_conformance_b" => {
            let metadata_id = catalog
                .get(b"Metadata")
                .expect("canonical metadata")
                .as_reference()
                .expect("indirect canonical metadata");
            let metadata = document
                .get_object_mut(metadata_id)
                .expect("canonical metadata object")
                .as_stream_mut()
                .expect("canonical metadata stream");
            let mut content = metadata
                .decompressed_content()
                .expect("decompress canonical metadata");
            replace_once(
                &mut content,
                b"<pdfaid:conformance>A</pdfaid:conformance>",
                b"<pdfaid:conformance>B</pdfaid:conformance>",
            );
            metadata.set_content(content);
        }
        "tagged_missing" => {
            document
                .get_object_mut(root_id)
                .expect("canonical catalog object")
                .as_dict_mut()
                .expect("canonical catalog dictionary")
                .remove(b"MarkInfo");
        }
        "struct_tree_missing" => {
            document
                .get_object_mut(root_id)
                .expect("canonical catalog object")
                .as_dict_mut()
                .expect("canonical catalog dictionary")
                .remove(b"StructTreeRoot");
        }
        "role_map_wrong_type" | "role_map_cycle" => {
            let struct_tree_id = catalog
                .get(b"StructTreeRoot")
                .expect("canonical structure tree")
                .as_reference()
                .expect("indirect canonical structure tree");
            let struct_tree = document
                .get_object_mut(struct_tree_id)
                .expect("canonical structure tree object")
                .as_dict_mut()
                .expect("canonical structure tree dictionary");
            if case == "role_map_wrong_type" {
                struct_tree.set("RoleMap", Object::Integer(1));
            } else {
                struct_tree.set(
                    "RoleMap",
                    dictionary! {"CycleA" => "CycleB", "CycleB" => "CycleA"},
                );
                let element_id = document
                    .objects
                    .iter()
                    .find_map(|(object_id, object)| {
                        let dictionary = object.as_dict().ok()?;
                        (dictionary.get(b"Type").ok()?.as_name().ok()? == b"StructElem")
                            .then_some(*object_id)
                    })
                    .expect("canonical structure element");
                document
                    .get_object_mut(element_id)
                    .expect("canonical structure element object")
                    .as_dict_mut()
                    .expect("canonical structure element dictionary")
                    .set("S", Object::Name(b"CycleA".to_vec()));
            }
        }
        "language_missing" => {
            let element_id = document
                .objects
                .iter()
                .find_map(|(object_id, object)| {
                    let dictionary = object.as_dict().ok()?;
                    (dictionary.get(b"Type").ok()?.as_name().ok()? == b"StructElem")
                        .then_some(*object_id)
                })
                .expect("canonical structure element");
            document
                .get_object_mut(element_id)
                .expect("canonical structure element object")
                .as_dict_mut()
                .expect("canonical structure element dictionary")
                .set("Lang", Object::string_literal("fr--CA"));
        }
        "unicode_missing" => {
            let font_id = document
                .objects
                .iter()
                .find_map(|(object_id, object)| {
                    let dictionary = object.as_dict().ok()?;
                    let base_font = dictionary.get(b"BaseFont").ok()?.as_name().ok()?;
                    (dictionary.get(b"Subtype").ok()?.as_name().ok()? == b"Type0"
                        && base_font
                            .windows(b"Regular".len())
                            .any(|window| window == b"Regular")
                        && dictionary.get(b"ToUnicode").is_ok())
                    .then_some(*object_id)
                })
                .expect("canonical Type0 font with ToUnicode");
            let encoding = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "CMap",
                    "CMapName" => "CanonicalIdentity",
                    "CIDSystemInfo" => dictionary! {
                        "Registry" => Object::string_literal("Adobe"),
                        "Ordering" => Object::string_literal("Identity"),
                        "Supplement" => 0,
                    },
                    "WMode" => 0,
                },
                embedded_identity_usecmap("Identity", 0),
            ));
            let font = document
                .get_object_mut(font_id)
                .expect("canonical font object")
                .as_dict_mut()
                .expect("canonical font dictionary");
            font.set("Encoding", encoding);
            font.remove(b"ToUnicode");
        }
        _ => panic!("unknown canonical A-1a mutation {case}"),
    }

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save canonical mutation");
    bytes
}

fn replace_once(bytes: &mut Vec<u8>, from: &[u8], to: &[u8]) {
    let offset = bytes
        .windows(from.len())
        .position(|window| window == from)
        .expect("canonical mutation source bytes");
    bytes.splice(offset..offset + from.len(), to.iter().copied());
}

pub fn font_fixture(case: &str) -> Vec<u8> {
    if matches!(
        case,
        "unicode_type0_identity_h" | "unicode_type0_identity_v" | "unicode_type0_gb1"
    ) {
        return type0_descendant_fixture(case);
    }
    font_fixture_with_type1_program(case, None)
}

pub fn font_fixture_with_external_type1_program(case: &str, program: &[u8]) -> Vec<u8> {
    font_fixture_with_type1_program(case, Some(program))
}

pub fn font_fixture_with_type1_program(
    case: &str,
    external_type1_program: Option<&[u8]>,
) -> Vec<u8> {
    let mut document = pdf_document();
    let pages_id = document.new_object_id();
    let embedded = !matches!(
        case,
        "unembedded_visible"
            | "unembedded_invisible"
            | "mixed_rendering_modes"
            | "mixed_visible_first"
            | "unused_resource"
            | "selected_not_shown"
            | "direct_font"
            | "form_unembedded"
            | "form_unembedded_indirect_subtype"
            | "nested_form_unembedded"
            | "inherited_resources"
            | "repeated_aliases"
            | "two_unembedded_fonts"
            | "type0_unembedded_descendant"
            | "missing_descriptor"
            | "malformed_descriptor"
            | "graphics_state_visible"
            | "graphics_state_invisible"
            | "cyclic_form"
            | "large_content"
            | "font_subtype_indirect_unembedded"
    );
    let mut descriptor = font_descriptor(&mut document, embedded);
    if matches!(
        case,
        "composite_baseline"
            | "composite_identity_v"
            | "composite_indirect_identity_h"
            | "composite_cidmap_indirect_identity"
            | "composite_cid_subset_missing_cidset"
            | "type0_embedded_descendant"
    ) {
        descriptor.set(
            "FontFile2",
            document.add_object(Stream::new(
                Dictionary::new(),
                sfnt::minimal_truetype_with_glyph_count(33),
            )),
        );
    }
    if case.starts_with("tt_symbolic_") {
        descriptor.set("Flags", 4);
    }
    if case == "tt_symbolic_indirect_flags" {
        // An *indirect* reference to the Symbolic flags value (4), not a
        // direct one -- confirmed live against veraPDF 1.30.2 to be
        // resolved and treated as symbolic exactly like a direct value.
        let indirect_flags = document.add_object(Object::Integer(4));
        descriptor.set("Flags", indirect_flags);
    }
    if matches!(
        case,
        "tt_symbolic_two_cmaps"
            | "tt_symbolic_two_cmaps_with_cmap30"
            | "tt_nonsymbolic_zero_cmaps"
            | "tt_nonsymbolic_one_cmap30"
    ) {
        descriptor.set(
            "FontFile2",
            document.add_object(Stream::new(
                Dictionary::new(),
                match case {
                    "tt_symbolic_two_cmaps_with_cmap30" => {
                        sfnt::minimal_truetype_with_symbol_cmap(2)
                    }
                    "tt_nonsymbolic_zero_cmaps" => sfnt::minimal_truetype_with_cmap_count(0),
                    "tt_nonsymbolic_one_cmap30" => sfnt::minimal_truetype_with_symbol_cmap(1),
                    _ => sfnt::minimal_truetype_with_cmap_count(2),
                },
            )),
        );
    }
    if matches!(
        case,
        "tt_nonascii_winansi" | "tt_nonascii_winansi_width_mismatch"
    ) {
        descriptor.set(
            "FontFile2",
            document.add_object(Stream::new(
                Dictionary::new(),
                sfnt::minimal_truetype_with_cmap_mapping(0xe9),
            )),
        );
    }
    if case == "direct_font_file" {
        descriptor.set(
            "FontFile2",
            Object::Stream(Stream::new(Dictionary::new(), sfnt::minimal_truetype())),
        );
    } else if case == "malformed_font_program" {
        descriptor.set(
            "FontFile2",
            document.add_object(Stream::new(Dictionary::new(), b"not a font".to_vec())),
        );
    } else if case == "malformed_font_file" {
        descriptor.set("FontFile2", 42);
    } else if case == "missing_font_file_object" {
        descriptor.set("FontFile2", Object::Reference((999_999, 0)));
    } else if case == "composite_cidmap_missing_unembedded" {
        descriptor.remove(b"FontFile2");
    } else if case == "font_file_subtype_invalid" {
        descriptor.set(
            "FontFile2",
            document.add_object(Stream::new(
                dictionary! {
                    "Subtype" => "OpenType",
                },
                sfnt::minimal_truetype(),
            )),
        );
    } else if case == "font_file_subtype_invalid_fontfile3" {
        descriptor.remove(b"FontFile2");
        descriptor.set(
            "FontFile3",
            document.add_object(Stream::new(
                dictionary! { "Subtype" => "Type1" },
                minimal_type1c(true),
            )),
        );
    } else if case == "type1_fontfile_header_only_garbage" {
        // A /FontFile whose bytes start with the Type1 magic header
        // (`%!PS-AdobeFont`) but are otherwise garbage, not a real
        // eexec-encrypted program -- tests whether veraPDF's own
        // containsFontFile check for /FontFile requires more structural
        // validity than the local magic-byte heuristic does.
        descriptor.remove(b"FontFile2");
        descriptor.set(
            "FontFile",
            document.add_object(Stream::new(
                Dictionary::new(),
                b"%!PS-AdobeFont-1.0: garbage garbage garbage not a real program".to_vec(),
            )),
        );
    } else if case.starts_with("type1_real_symbol_")
        || matches!(case, "unicode_type1_standard" | "unicode_type1_symbol")
    {
        let (program, length1, length2, length3) = pdf_type1_program(
            external_type1_program.unwrap_or(include_bytes!("../fixtures/fonts/usyr.pfa")),
        );
        descriptor.remove(b"FontFile2");
        descriptor.set("FontName", "StandardSymL");
        descriptor.set(
            "FontFile",
            document.add_object(Stream::new(
                dictionary! {
                    "Length1" => i64::try_from(length1).expect("Type1 clear length"),
                    "Length2" => i64::try_from(length2).expect("Type1 encrypted length"),
                    "Length3" => i64::try_from(length3).expect("Type1 trailer length"),
                },
                program,
            )),
        );
        if matches!(
            case,
            "type1_real_symbol_subset_complete"
                | "type1_real_symbol_subset_incomplete"
                | "type1_real_symbol_subset_program_encoding_ignored"
        ) {
            descriptor.set(
                "CharSet",
                Object::string_literal(if case == "type1_real_symbol_subset_complete" {
                    "/.notdef/universal"
                } else {
                    "/.notdef"
                }),
            );
        }
    } else if matches!(
        case,
        "type1_glyph_missing"
            | "type1_glyph_present"
            | "type1_difference_glyph"
            | "type1_subset_charset_incomplete"
            | "type1_subset_charset_difference_incomplete"
            | "type1_subset_charset_incomplete_indirect_basefont"
            | "type1_width_mismatch"
    ) {
        descriptor.remove(b"FontFile2");
        descriptor.set(
            "FontFile",
            document.add_object(Stream::new(
                Dictionary::new(),
                if case == "type1_width_mismatch" {
                    type1_program_with_width(500)
                } else {
                    type1_program(&["space"])
                },
            )),
        );
        if matches!(
            case,
            "type1_subset_charset_incomplete"
                | "type1_subset_charset_difference_incomplete"
                | "type1_subset_charset_incomplete_indirect_basefont"
        ) {
            descriptor.set("CharSet", Object::string_literal("/.notdef"));
        }
    } else if matches!(
        case,
        "type1c_glyph_missing"
            | "type1c_glyph_present"
            | "type1c_default_charset_space"
            | "type1c_width_mismatch"
            | "type1c_subset_complete"
            | "type1c_subset_incomplete"
            | "type1c_subset_program_encoding_ignored"
    ) {
        descriptor.remove(b"FontFile2");
        descriptor.set(
            "FontFile3",
            document.add_object(Stream::new(
                dictionary! {
                    "Subtype" => "Type1C",
                },
                if case == "type1c_default_charset_space" {
                    minimal_type1c_default_charset(true)
                } else {
                    minimal_type1c(case != "type1c_glyph_missing")
                },
            )),
        );
        if matches!(
            case,
            "type1c_subset_complete"
                | "type1c_subset_incomplete"
                | "type1c_subset_program_encoding_ignored"
        ) {
            descriptor.set(
                "CharSet",
                Object::string_literal(if case == "type1c_subset_complete" {
                    "/.notdef/space"
                } else {
                    "/.notdef"
                }),
            );
        }
    } else if case == "type1c_header_only_garbage" {
        // A /FontFile3 whose bytes satisfy the local header heuristic
        // (major version 1 or 2, hdrSize <= length) but are otherwise
        // garbage, not a real CFF table -- tests whether veraPDF's own
        // containsFontFile check for /FontFile3 requires more structural
        // validity than the local header-byte heuristic does.
        descriptor.remove(b"FontFile2");
        descriptor.set(
            "FontFile3",
            document.add_object(Stream::new(
                dictionary! {
                    "Subtype" => "Type1C",
                },
                vec![1, 0, 4, 4, 0xFF, 0xFF, 0xFF, 0xFF],
            )),
        );
    } else if case == "type1c_embedded_indirect_subtype" {
        // The embedded FontFile3 stream's own /Subtype as an *indirect*
        // reference to the name /Type1C, not a direct one -- discriminates
        // whether an unresolved indirect /Subtype wrongly makes a validly
        // embedded Type1C program count as unembedded.
        descriptor.remove(b"FontFile2");
        let indirect_subtype = document.add_object(Object::Name(b"Type1C".to_vec()));
        descriptor.set(
            "FontFile3",
            document.add_object(Stream::new(
                dictionary! {
                    "Subtype" => indirect_subtype,
                },
                minimal_type1c(true),
            )),
        );
    } else if matches!(
        case,
        "composite_cff_missing_glyph"
            | "composite_cff_present_glyph"
            | "composite_cff_width_mismatch"
            | "composite_cff_cidset_missing"
    ) {
        descriptor.set(
            "FontFile3",
            document.add_object(Stream::new(
                dictionary! {
                    "Subtype" => "CIDFontType0C",
                },
                minimal_cidfonttype0c(case != "composite_cff_missing_glyph"),
            )),
        );
        let cid_set = if case == "composite_cff_cidset_missing" {
            vec![0; 5]
        } else {
            vec![0, 0, 0, 0, 0x80]
        };
        descriptor.set(
            "CIDSet",
            document.add_object(Stream::new(Dictionary::new(), cid_set)),
        );
    } else if matches!(
        case,
        "composite_cidset_real_program"
            | "composite_cidset_nonidentity_real_program"
            | "composite_named_cmap_cidset_real_program"
            | "composite_cidset_indirect_basefont_real_program"
    ) {
        descriptor.set(
            "FontFile2",
            document.add_object(Stream::new(
                Dictionary::new(),
                sfnt::minimal_truetype_with_glyph_count(33),
            )),
        );
        descriptor.set(
            "CIDSet",
            document.add_object(Stream::new(Dictionary::new(), vec![0; 5])),
        );
    } else if matches!(
        case,
        "composite_identity_width_mismatch"
            | "composite_identity_width_override_mismatch"
            | "composite_descendant_subtype_indirect_width_mismatch"
            | "composite_dw_indirect_mismatch"
            | "composite_w_singles_element_indirect_mismatch"
    ) {
        descriptor.set(
            "FontFile2",
            document.add_object(Stream::new(
                Dictionary::new(),
                sfnt::minimal_truetype_with_glyph_count(33),
            )),
        );
    } else if matches!(
        case,
        "composite_stream_cidmap_missing_glyph"
            | "composite_nonidentity_multibyte_missing_glyph"
            | "composite_identity_usecmap_missing_glyph"
    ) {
        descriptor.set(
            "FontFile2",
            document.add_object(Stream::new(
                Dictionary::new(),
                sfnt::minimal_truetype_with_glyph_count(2),
            )),
        );
    }
    let descriptor_object = if matches!(case, "direct_descriptor" | "direct_font_file") {
        Object::Dictionary(descriptor)
    } else {
        Object::Reference(document.add_object(descriptor))
    };
    let mut font = dictionary! {
       "Type" => "Font",
       "Subtype" => "TrueType",
       "BaseFont" => "MaiTestFont",
       "Encoding" => "WinAnsiEncoding",
       "FirstChar" => 32,
       "LastChar" => 32,
       "Widths" => vec![500.into()],
       "FontDescriptor" => descriptor_object.clone(),
    };
    if case == "missing_descriptor" {
        font.remove(b"FontDescriptor");
    } else if case == "malformed_descriptor" {
        font.set("FontDescriptor", 42);
    } else if case == "type1_subset_missing_charset" {
        font.set("Subtype", "Type1");
        font.set("BaseFont", "ABCDEF+MaiTestFont");
    } else if matches!(
        case,
        "font_file_subtype_invalid_fontfile3" | "type1_fontfile_header_only_garbage"
    ) {
        font.set("Subtype", "Type1");
    } else if case.starts_with("type1_real_symbol_")
        || matches!(case, "unicode_type1_standard" | "unicode_type1_symbol")
    {
        font.set("Subtype", "Type1");
        font.set(
            "BaseFont",
            if matches!(
                case,
                "type1_real_symbol_subset_complete"
                    | "type1_real_symbol_subset_incomplete"
                    | "type1_real_symbol_subset_program_encoding_ignored"
            ) {
                "ABCDEF+StandardSymL"
            } else {
                "StandardSymL"
            },
        );
        font.remove(b"Encoding");
        font.set("FirstChar", 34);
        font.set("LastChar", 34);
        font.set(
            "Widths",
            vec![if case == "type1_real_symbol_width_mismatch" {
                700.into()
            } else {
                713.into()
            }],
        );
        if case == "type1_real_symbol_pdf_base_missing_glyph" {
            font.set("Encoding", "WinAnsiEncoding");
        } else if matches!(
            case,
            "type1_real_symbol_difference_present"
                | "type1_real_symbol_subset_complete"
                | "type1_real_symbol_subset_incomplete"
        ) {
            font.set(
                "Encoding",
                dictionary! {
                    "Differences" => vec![34.into(), Object::Name(b"universal".to_vec())],
                },
            );
        }
    } else if matches!(
        case,
        "type1_glyph_missing"
            | "type1_glyph_present"
            | "type1_difference_glyph"
            | "type1_indirect_difference_code"
            | "type1_subset_charset_difference_incomplete"
            | "type1_width_mismatch"
            | "type1c_glyph_missing"
            | "type1c_glyph_present"
            | "type1c_default_charset_space"
            | "type1c_width_mismatch"
            | "type1c_subset_complete"
            | "type1c_subset_incomplete"
            | "type1c_subset_program_encoding_ignored"
            | "type1c_embedded_indirect_subtype"
            | "type1c_header_only_garbage"
    ) {
        font.set("Subtype", "Type1");
        if case == "type1_subset_charset_difference_incomplete" {
            font.set("BaseFont", "ABCDEF+MaiTestFont");
        }
        if matches!(
            case,
            "type1c_glyph_missing"
                | "type1c_glyph_present"
                | "type1c_default_charset_space"
                | "type1c_subset_complete"
                | "type1c_subset_incomplete"
                | "type1c_subset_program_encoding_ignored"
                | "type1c_embedded_indirect_subtype"
                | "type1c_header_only_garbage"
        ) {
            font.set("Widths", vec![0.into()]);
        }
        if matches!(
            case,
            "type1c_subset_complete"
                | "type1c_subset_incomplete"
                | "type1c_subset_program_encoding_ignored"
        ) {
            font.set("BaseFont", "ABCDEF+MaiTestFont");
        }
        if case == "type1c_subset_program_encoding_ignored" {
            font.remove(b"Encoding");
        }
        if case == "type1c_default_charset_space" {
            font.set(
                "Encoding",
                dictionary! {
                    "Differences" => vec![32.into(), Object::Name(b"space".to_vec())],
                },
            );
        }
        if matches!(
            case,
            "type1_difference_glyph" | "type1_subset_charset_difference_incomplete"
        ) {
            font.set(
                "Encoding",
                dictionary! {
                    "Differences" => Object::Array(vec![33.into(), Object::Name(b"space".to_vec())]),
                },
            );
        }
        if case == "type1_indirect_difference_code" {
            // The Differences array's code entry (33) as an *indirect*
            // reference, not a direct integer -- confirmed live against
            // veraPDF 1.30.2 to be resolved and used exactly like a direct
            // value.
            let indirect_code = document.add_object(Object::Integer(33));
            font.set(
                "Encoding",
                dictionary! {
                    "Differences" => Object::Array(vec![
                        Object::Reference(indirect_code),
                        Object::Name(b"space".to_vec()),
                    ]),
                },
            );
        }
        if case == "type1_width_mismatch" {
            font.set("Widths", vec![400.into()]);
        }
    } else if matches!(
        case,
        "type1_subset_charset_incomplete" | "type1_subset_charset_difference_incomplete"
    ) {
        font.set("Subtype", "Type1");
        font.set("BaseFont", "ABCDEF+MaiTestFont");
    } else if case == "type1_subset_charset_incomplete_indirect_basefont" {
        // /BaseFont as an *indirect* reference to a subset-tagged name, not
        // a direct one -- discriminates whether an unresolved indirect
        // /BaseFont wrongly makes the font look non-subset (both the
        // TYPE1-SUBSET-CHARSET-001 dictionary check and the rendered-glyph
        // check silently skipped) instead of being resolved and recognized.
        font.set("Subtype", "Type1");
        let indirect_base_font = document.add_object(Object::Name(b"ABCDEF+MaiTestFont".to_vec()));
        font.set("BaseFont", indirect_base_font);
    }
    if matches!(
        case,
        "unicode_name_basefont_invalid"
            | "unicode_name_basefont_unused"
            | "unicode_name_basefont_indirect"
            | "unicode_name_basefont_unreferenced"
    ) {
        let invalid_name = Object::Name(b"MaiTest\xffFont".to_vec());
        font.set(
            "BaseFont",
            if case == "unicode_name_basefont_indirect" {
                Object::Reference(document.add_object(invalid_name))
            } else {
                invalid_name
            },
        );
    }
    if matches!(
        case,
        "type3_visible"
            | "type3_width_match"
            | "type3_width_mismatch"
            | "type3_width_d1_mismatch"
            | "type3_width_tolerance_boundary"
            | "type3_missing_charproc_zero_width"
            | "type3_macroman_base"
            | "type3_macexpert_base"
            | "type3_notdef"
            | "unicode_missing"
            | "unicode_scalar"
            | "unicode_indirect"
            | "unicode_malformed"
            | "unicode_incomplete"
    ) {
        let mut char_procs = Dictionary::new();
        let charproc_bytes = match case {
            "type3_width_match"
            | "type3_macroman_base"
            | "type3_macexpert_base"
            | "type3_notdef" => Some(b"500 0 d0\n".as_slice()),
            "type3_width_tolerance_boundary" => Some(b"499 0 d0\n".as_slice()),
            "type3_width_mismatch" => Some(b"400 0 d0\n".as_slice()),
            "type3_width_d1_mismatch" => Some(b"400 0 0 0 500 700 d1\n".as_slice()),
            _ => None,
        };
        if let Some(bytes) = charproc_bytes {
            char_procs.set(
                if case == "type3_macexpert_base" {
                    "exclamsmall"
                } else if case == "type3_notdef" {
                    ".notdef"
                } else {
                    "space"
                },
                document.add_object(Stream::new(Dictionary::new(), bytes.to_vec())),
            );
        }
        let encoding = match case {
            "type3_macroman_base" | "unicode_macroman" => {
                Object::Name(b"MacRomanEncoding".to_vec())
            }
            "type3_macexpert_base" | "unicode_macexpert" => {
                Object::Name(b"MacExpertEncoding".to_vec())
            }
            "unicode_winansi" => Object::Name(b"WinAnsiEncoding".to_vec()),
            "type3_notdef" => Object::Dictionary(dictionary! {
                "Type" => "Encoding",
                "Differences" => vec![32.into(), Object::Name(b".notdef".to_vec())],
            }),
            _ => Object::Dictionary(dictionary! {
                "Type" => "Encoding",
                "Differences" => vec![32.into(), Object::Name(b"space".to_vec())],
            }),
        };
        let rendered_byte = match case {
            "type3_macroman_base" => 202,
            "type3_macexpert_base" => 33,
            _ => 32,
        };
        font = dictionary! {
            "Type" => "Font",
            "Subtype" => "Type3",
            "FontBBox" => vec![0.into(), 0.into(), 500.into(), 700.into()],
            "FontMatrix" => vec![0.001.into(), 0.into(), 0.into(), 0.001.into(), 0.into(), 0.into()],
            "CharProcs" => char_procs,
            "Encoding" => encoding,
            "FirstChar" => rendered_byte,
            "LastChar" => rendered_byte,
            "Widths" => vec![if case == "type3_missing_charproc_zero_width" {
                0.into()
            } else {
                500.into()
            }],
        };
    } else if matches!(
        case,
        "type0_unembedded_descendant" | "type0_embedded_descendant"
    ) || case.starts_with("composite_")
    {
        let mut descendant_dictionary = dictionary! {
            "Type" => "Font",
            "Subtype" => "CIDFontType2",
            "BaseFont" => if matches!(
                case,
                "composite_cid_subset_missing_cidset"
                    | "composite_cidset_real_program"
                    | "composite_cidset_nonidentity_real_program"
                    | "composite_named_cmap_cidset_real_program"
                    | "composite_cidset_indirect_basefont_real_program"
                    | "composite_cff_missing_glyph"
                    | "composite_cff_present_glyph"
                    | "composite_cff_width_mismatch"
                    | "composite_cff_cidset_missing"
            ) {
                Object::Name(b"ABCDEF+MaiTestFont".to_vec())
            } else {
                Object::Name(b"MaiTestFont".to_vec())
            },
            "CIDSystemInfo" => dictionary! {
                "Registry" => Object::string_literal("Adobe"),
                "Ordering" => Object::string_literal("Identity"),
                "Supplement" => 0,
            },
            "FontDescriptor" => descriptor_object,
            "DW" => 500,
            "CIDToGIDMap" => "Identity",
        };
        if case == "composite_indirect_cid_system_info" {
            // The descendant's CIDSystemInfo /Registry as an *indirect*
            // reference, not a direct string -- confirmed live against
            // veraPDF 1.30.2 to be resolved and compared exactly like a
            // direct value.
            let indirect_registry = document.add_object(Object::string_literal("Adobe"));
            descendant_dictionary.set(
                "CIDSystemInfo",
                dictionary! {
                    "Registry" => indirect_registry,
                    "Ordering" => Object::string_literal("Identity"),
                    "Supplement" => 0,
                },
            );
        }
        if case == "composite_named_cmap_cidset_real_program" {
            descendant_dictionary.set(
                "CIDSystemInfo",
                dictionary! {
                    "Registry" => Object::string_literal("Adobe"),
                    "Ordering" => Object::string_literal("Japan1"),
                    "Supplement" => 4,
                },
            );
        }
        if case == "composite_cidset_indirect_basefont_real_program" {
            // The descendant's own /BaseFont as an *indirect* reference to
            // a subset-tagged name, not a direct one -- discriminates
            // whether an unresolved indirect /BaseFont wrongly makes the
            // descendant look non-subset (both the dict-level /CIDSet
            // presence check and the rendered-CID coverage check silently
            // skipped) instead of being resolved and recognized.
            let indirect_base_font =
                document.add_object(Object::Name(b"ABCDEF+MaiTestFont".to_vec()));
            descendant_dictionary.set("BaseFont", indirect_base_font);
        }
        if case == "composite_descendant_subtype_indirect_width_mismatch" {
            // The descendant's own /Subtype as an *indirect* reference to
            // the name /CIDFontType2, not a direct one, paired with a
            // genuine /DW mismatch (400 vs. the embedded program's real
            // 500) so only a resolved, recognized Subtype proves the
            // descendant's glyph/width checks still run rather than being
            // silently skipped as an unrecognized subtype.
            let indirect_subtype = document.add_object(Object::Name(b"CIDFontType2".to_vec()));
            descendant_dictionary.set("Subtype", indirect_subtype);
        }
        if matches!(
            case,
            "composite_cff_missing_glyph"
                | "composite_cff_present_glyph"
                | "composite_cff_width_mismatch"
                | "composite_cff_cidset_missing"
        ) {
            descendant_dictionary.set("Subtype", "CIDFontType0");
            descendant_dictionary.remove(b"CIDToGIDMap");
            if case == "composite_cff_width_mismatch" {
                descendant_dictionary.set("DW", 500);
            } else {
                descendant_dictionary.set("DW", 0);
            }
        }
        if case == "composite_cmap_supplement_mismatch" {
            descendant_dictionary.set(
                "CIDSystemInfo",
                dictionary! {
                    "Registry" => Object::string_literal("Adobe"),
                    "Ordering" => Object::string_literal("Identity"),
                    "Supplement" => 1,
                },
            );
        }
        match case {
            "composite_cidmap_missing" | "composite_cidmap_missing_unembedded" => {
                descendant_dictionary.remove(b"CIDToGIDMap");
            }
            // The descendant's own /Subtype as an *indirect* reference to
            // /CIDFontType2, not a direct one, paired with a missing
            // /CIDToGIDMap -- discriminates whether an unresolved indirect
            // /Subtype wrongly skips the /CIDToGIDMap validity check
            // entirely instead of being resolved and recognized.
            "composite_cidmap_missing_indirect_subtype" => {
                descendant_dictionary.remove(b"CIDToGIDMap");
                let indirect_subtype = document.add_object(Object::Name(b"CIDFontType2".to_vec()));
                descendant_dictionary.set("Subtype", indirect_subtype);
            }
            "composite_cidmap_invalid_name" => {
                descendant_dictionary.set("CIDToGIDMap", "NotIdentity");
            }
            "composite_cidmap_stream" => {
                let map = document.add_object(Stream::new(Dictionary::new(), vec![0, 0]));
                descendant_dictionary.set("CIDToGIDMap", map);
            }
            // An *indirect* reference to the name /Identity, not a direct
            // one -- confirmed live against veraPDF 1.30.2 to be accepted
            // exactly like a direct /Identity.
            "composite_cidmap_indirect_identity" => {
                let map = document.add_object(Object::Name(b"Identity".to_vec()));
                descendant_dictionary.set("CIDToGIDMap", map);
            }
            "composite_stream_cidmap_missing_glyph" => {
                let mut map = vec![0; 66];
                *map.get_mut(65).expect("CIDToGIDMap missing-glyph slot") = 2;
                let map = document.add_object(Stream::new(Dictionary::new(), map));
                descendant_dictionary.set("CIDToGIDMap", map);
            }
            "composite_identity_width_mismatch"
            | "composite_descendant_subtype_indirect_width_mismatch" => {
                descendant_dictionary.set("DW", 400);
            }
            // /DW as an *indirect* reference to a mismatched value (400
            // vs. the embedded program's real 500), not a direct one.
            "composite_dw_indirect_mismatch" => {
                let indirect_dw = document.add_object(Object::Integer(400));
                descendant_dictionary.set("DW", indirect_dw);
            }
            "composite_identity_width_override_mismatch" => {
                descendant_dictionary.set(
                    "W",
                    Object::Array(vec![32.into(), Object::Array(vec![400.into()])]),
                );
            }
            // A /W singles group (`c [w1 ...]`) whose one width entry is an
            // *indirect* reference to a mismatched value, not a direct one.
            "composite_w_singles_element_indirect_mismatch" => {
                let indirect_width = document.add_object(Object::Integer(400));
                descendant_dictionary.set(
                    "W",
                    Object::Array(vec![
                        32.into(),
                        Object::Array(vec![Object::Reference(indirect_width)]),
                    ]),
                );
            }
            _ => {}
        }
        let descendant = document.add_object(descendant_dictionary);
        let encoding = match case {
            "composite_identity_v" => Object::Name(b"Identity-V".to_vec()),
            // An *indirect* reference to the name /Identity-H, not a direct
            // one -- confirmed live against veraPDF 1.30.2 to be accepted
            // exactly like a direct /Identity-H.
            "composite_indirect_identity_h" => {
                Object::Reference(document.add_object(Object::Name(b"Identity-H".to_vec())))
            }
            "composite_named_cmap" | "composite_named_cmap_cidset_real_program" => {
                Object::Name(b"UniJIS-UCS2-H".to_vec())
            }
            "composite_unknown_named_cmap" => Object::Name(b"NotAStandardCMap".to_vec()),
            "composite_cmap_matching"
            | "composite_cmap_supplement_mismatch"
            | "composite_cmap_mismatch_system"
            | "composite_indirect_cid_system_info"
            | "composite_cmap_wmode_match"
            | "composite_cmap_wmode_mismatch"
            | "composite_cmap_wmode_indirect_match"
            | "composite_cmap_cid_too_large"
            | "composite_cmap_unknown_usecmap"
            | "composite_cmap_dictionary_unknown_usecmap"
            | "composite_cidset_nonidentity_real_program"
            | "composite_nonidentity_missing_glyph"
            | "composite_nonidentity_multibyte_missing_glyph"
            | "composite_identity_usecmap_missing_glyph" => {
                let cmap_ordering = if case == "composite_cmap_mismatch_system" {
                    "Japan1"
                } else {
                    "Identity"
                };
                let dictionary_wmode = i64::from(case == "composite_cmap_wmode_match")
                    + i64::from(case == "composite_cmap_wmode_mismatch")
                    + i64::from(case == "composite_cmap_wmode_indirect_match");
                let content_wmode = i64::from(case == "composite_cmap_wmode_match")
                    + i64::from(case == "composite_cmap_wmode_indirect_match");
                let cid_start = u32::from(case == "composite_cmap_cid_too_large") * 65_536;
                // An *indirect* reference to the /WMode integer, not a
                // direct one -- confirmed live against veraPDF 1.30.2 to be
                // resolved and compared exactly like a direct value.
                let wmode_object = if case == "composite_cmap_wmode_indirect_match" {
                    Object::Reference(document.add_object(Object::Integer(dictionary_wmode)))
                } else {
                    Object::Integer(dictionary_wmode)
                };
                let mut cmap_dictionary = dictionary! {
                    "Type" => "CMap",
                    "CMapName" => "Page-CMap",
                    "CIDSystemInfo" => dictionary! {
                        "Registry" => Object::string_literal("Adobe"),
                        "Ordering" => Object::string_literal(cmap_ordering),
                        "Supplement" => 0,
                    },
                    "WMode" => wmode_object,
                };
                if case == "composite_cmap_dictionary_unknown_usecmap" {
                    cmap_dictionary.set("UseCMap", Object::Name(b"NotAStandardCMap".to_vec()));
                }
                Object::Reference(document.add_object(Stream::new(
                    cmap_dictionary,
                    if case == "composite_nonidentity_multibyte_missing_glyph" {
                        embedded_two_byte_cmap(cmap_ordering, content_wmode, cid_start)
                    } else if case == "composite_identity_usecmap_missing_glyph" {
                        embedded_identity_usecmap(cmap_ordering, content_wmode)
                    } else if case == "composite_cmap_unknown_usecmap" {
                        embedded_unknown_usecmap(cmap_ordering, content_wmode)
                    } else {
                        embedded_cmap(cmap_ordering, content_wmode, cid_start)
                    },
                )))
            }
            _ => Object::Name(b"Identity-H".to_vec()),
        };
        font = dictionary! {
            "Type" => "Font",
            "Subtype" => "Type0",
            "BaseFont" => "MaiTestFont",
            "Encoding" => encoding,
            "DescendantFonts" => vec![Object::Reference(descendant)],
        };
    }
    match case {
        "font_type_missing" | "unused_invalid_font" => {
            font.remove(b"Type");
        }
        "font_type_invalid" => font.set("Type", "NotFont"),
        "font_subtype_missing" => {
            font.remove(b"Subtype");
        }
        "font_subtype_invalid" => font.set("Subtype", "UnsupportedFont"),
        // /Subtype as an *indirect* reference to the name /TrueType, on an
        // otherwise-unembedded font -- discriminates whether an unresolved
        // indirect /Subtype wrongly makes the whole font inapplicable (no
        // PDFont object, every predicate silently skipped) instead of being
        // resolved and recognized like a direct name.
        "font_subtype_indirect_unembedded" => {
            let indirect_subtype = document.add_object(Object::Name(b"TrueType".to_vec()));
            font.set("Subtype", indirect_subtype);
        }
        "font_basefont_missing" => {
            font.remove(b"BaseFont");
        }
        "font_basefont_invalid" => font.set("BaseFont", 42),
        "font_firstchar_missing" => {
            font.remove(b"FirstChar");
        }
        "font_lastchar_missing" => {
            font.remove(b"LastChar");
        }
        "font_widths_missing" => {
            font.remove(b"Widths");
        }
        "font_widths_wrong_size" => font.set("Widths", Vec::<Object>::new()),
        // The whole /Widths array as an *indirect* reference, not a direct
        // array, with a size and value that would otherwise fully comply.
        "font_widths_array_indirect" => {
            let indirect_widths = document.add_object(Object::Array(vec![500.into()]));
            font.set("Widths", indirect_widths);
        }
        // /Widths is direct, but its single entry is an *indirect* reference
        // to a mismatched value (497 vs. the embedded program's real 500),
        // so only a genuine, observable width discrepancy proves the
        // element itself was resolved and compared rather than silently
        // skipped.
        "font_widths_element_indirect_mismatch" => {
            let indirect_width = document.add_object(Object::Integer(497));
            font.set("Widths", vec![Object::Reference(indirect_width)]);
        }
        // /FirstChar and /LastChar as *indirect* references, not direct
        // integers -- confirmed live against veraPDF 1.30.2 to be resolved
        // and compared exactly like direct values.
        "font_firstchar_lastchar_indirect" => {
            let indirect_first_char = document.add_object(Object::Integer(32));
            let indirect_last_char = document.add_object(Object::Integer(32));
            font.set("FirstChar", indirect_first_char);
            font.set("LastChar", indirect_last_char);
        }
        "standard14_missing_metrics" => {
            font.set("Subtype", "Type1");
            font.set("BaseFont", "Helvetica");
            font.remove(b"FirstChar");
            font.remove(b"LastChar");
            font.remove(b"Widths");
            font.remove(b"FontDescriptor");
        }
        // The "standard 14 fonts" /FirstChar//LastChar//Widths exemption is
        // scoped to Type1/MMType1 only (confirmed live against veraPDF
        // 1.30.2): a TrueType font whose /BaseFont matches a standard-14
        // name (e.g. "Helvetica") still requires all three, unlike the
        // Type1 case above.
        "truetype_named_standard14_missing_metrics" => {
            font.set("BaseFont", "Helvetica");
            font.remove(b"FirstChar");
            font.remove(b"LastChar");
            font.remove(b"Widths");
            font.remove(b"FontDescriptor");
        }
        "tt_nonsymbolic_macroman" => font.set("Encoding", "MacRomanEncoding"),
        "tt_nonsymbolic_missing_encoding" => {
            font.remove(b"Encoding");
        }
        "tt_nonsymbolic_invalid_encoding" => font.set("Encoding", "StandardEncoding"),
        "tt_nonsymbolic_dictionary_winansi" => font.set(
            "Encoding",
            dictionary! {
                "Type" => "Encoding",
                "BaseEncoding" => "WinAnsiEncoding",
            },
        ),
        // /BaseEncoding as an *indirect* reference to the name
        // /WinAnsiEncoding, not a direct one -- discriminates whether an
        // unresolved indirect /BaseEncoding wrongly makes an otherwise
        // compliant non-symbolic encoding look unrecognized.
        "tt_nonsymbolic_dictionary_indirect_baseencoding" => {
            let indirect_base_encoding =
                document.add_object(Object::Name(b"WinAnsiEncoding".to_vec()));
            font.set(
                "Encoding",
                dictionary! {
                    "Type" => "Encoding",
                    "BaseEncoding" => indirect_base_encoding,
                },
            );
        }
        "tt_nonsymbolic_dictionary_macroman" => font.set(
            "Encoding",
            dictionary! {
                "Type" => "Encoding",
                "BaseEncoding" => "MacRomanEncoding",
            },
        ),
        "tt_nonsymbolic_differences" => font.set(
            "Encoding",
            dictionary! {
                "Type" => "Encoding",
                "BaseEncoding" => "WinAnsiEncoding",
                "Differences" => vec![32.into(), Object::Name(b"space".to_vec())],
            },
        ),
        "tt_nonsymbolic_differences_null" => font.set(
            "Encoding",
            dictionary! {
                "Type" => "Encoding",
                "BaseEncoding" => "WinAnsiEncoding",
                "Differences" => Object::Null,
            },
        ),
        "tt_glyph_width_mismatch" => font.set("Widths", vec![497.into()]),
        "tt_nonascii_winansi" => {
            font.set("FirstChar", 233);
            font.set("LastChar", 233);
            font.set("Widths", vec![500.into()]);
        }
        "tt_nonascii_winansi_width_mismatch" => {
            font.set("FirstChar", 233);
            font.set("LastChar", 233);
            font.set("Widths", vec![497.into()]);
        }
        "tt_symbolic_no_encoding"
        | "tt_symbolic_one_cmap"
        | "tt_symbolic_two_cmaps"
        | "tt_symbolic_two_cmaps_with_cmap30"
        | "tt_symbolic_indirect_flags" => {
            font.remove(b"Encoding");
        }
        // A symbolic font whose /Encoding is present but neither a name,
        // dictionary, nor null -- an integer. Hypothesis check: does
        // veraPDF's `Encoding == null` predicate fail here (since the value
        // is non-null), or does an unrecognized-shape value get silently
        // treated the same as absent, matching the local implementation?
        "tt_symbolic_malformed_encoding" => {
            font.set("Encoding", Object::Boolean(true));
        }
        "tt_nonsymbolic_malformed_encoding" => {
            font.set("Encoding", Object::Boolean(true));
        }
        _ => {}
    }
    if case.starts_with("unicode_") {
        let valid_cmap = || {
            Stream::new(
                dictionary! { "Type" => "CMap" },
                b"1 begincodespacerange <00> <ff> endcodespacerange 1 beginbfchar <20> <0020> endbfchar".to_vec(),
            )
        };
        match case {
            "unicode_missing" => {
                font.set(
                    "Encoding",
                    dictionary! {
                        "BaseEncoding" => "WinAnsiEncoding",
                        "Differences" => vec![32.into(), Object::Name(b"space".to_vec())],
                    },
                );
            }
            "unicode_scalar" => {
                font.set("Encoding", dictionary! { "Differences" => vec![32.into(), Object::Name(b"space".to_vec())] });
                font.set("ToUnicode", 1);
            }
            "unicode_indirect" => {
                font.set("Encoding", dictionary! { "Differences" => vec![32.into(), Object::Name(b"space".to_vec())] });
                font.set("ToUnicode", document.add_object(valid_cmap()));
            }
            "unicode_malformed" => {
                font.set("Encoding", dictionary! { "Differences" => vec![32.into(), Object::Name(b"space".to_vec())] });
                font.set(
                    "ToUnicode",
                    document.add_object(Stream::new(Dictionary::new(), b"not a CMap".to_vec())),
                );
            }
            "unicode_incomplete" => {
                font.set("Encoding", dictionary! { "Differences" => vec![32.into(), Object::Name(b"space".to_vec())] });
                font.set("ToUnicode", document.add_object(Stream::new(Dictionary::new(), b"1 begincodespacerange <00> <ff> endcodespacerange 1 beginbfchar <21> <0021> endbfchar".to_vec())));
            }
            "unicode_reserved" => {
                font.set("Encoding", dictionary! { "Differences" => vec![32.into(), Object::Name(b"space".to_vec())] });
                font.set("ToUnicode", document.add_object(Stream::new(Dictionary::new(), b"1 begincodespacerange <00> <ff> endcodespacerange 1 beginbfchar <20> <0000> endbfchar".to_vec())));
            }
            "unicode_pua_missing_actual_text"
            | "unicode_pua_with_actual_text"
            | "unicode_pua_null_actual_text"
            | "unicode_pua_integer_actual_text"
            | "unicode_pua_indirect_actual_text"
            | "unicode_pua_named_actual_text"
            | "unicode_pua_invisible_missing_actual_text" => {
                font.set(
                    "ToUnicode",
                    document.add_object(Stream::new(
                        Dictionary::new(),
                        b"1 begincodespacerange <00> <ff> endcodespacerange 1 beginbfchar <20> <E000> endbfchar".to_vec(),
                    )),
                );
            }
            "unicode_winansi" => font.set("Encoding", "WinAnsiEncoding"),
            "unicode_macroman" => font.set("Encoding", "MacRomanEncoding"),
            "unicode_macexpert" => font.set("Encoding", "MacExpertEncoding"),
            "unicode_type1_standard" => {
                font.set("Subtype", "Type1");
                font.remove(b"Encoding");
            }
            "unicode_type1_symbol" => {
                font.set("Subtype", "Type1");
                font.set("BaseFont", "Symbol");
                font.remove(b"Encoding");
            }
            _ => {}
        }
    }
    let font_object = if case == "direct_font" {
        Object::Dictionary(font)
    } else {
        Object::Reference(document.add_object(font))
    };

    let mut font_resources = dictionary! {
        "F1" => font_object.clone(),
    };
    if case == "repeated_aliases" {
        font_resources.set("F2", font_object.clone());
    }
    if case == "two_unembedded_fonts" {
        let descriptor = font_descriptor(&mut document, false);
        let second_descriptor = document.add_object(descriptor);
        let second_font = document.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "TrueType",
            "BaseFont" => "MaiSecondFont",
            "Encoding" => "WinAnsiEncoding",
            "FirstChar" => 32,
            "LastChar" => 32,
            "Widths" => vec![500.into()],
            "FontDescriptor" => second_descriptor,
        });
        font_resources.set("F2", second_font);
    }
    let mut resources = dictionary! {
        "Font" => dictionary! {
            "F1" => font_object,
        },
    };
    resources.set("Font", font_resources.clone());
    if case == "unicode_pua_named_actual_text" {
        resources.set(
            "Properties",
            dictionary! {
                "P" => dictionary! { "ActualText" => Object::string_literal("replacement") },
            },
        );
    }
    if case == "unicode_name_basefont_unreferenced" {
        resources.remove(b"Font");
    }
    let page_content = match case {
        case if case.starts_with("type1_real_symbol_")
            || matches!(case, "unicode_type1_standard" | "unicode_type1_symbol") =>
        {
            content(vec![
                operation("BT", vec![]),
                operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
                operation(
                    "Tj",
                    vec![Object::String(vec![34], lopdf::StringFormat::Literal)],
                ),
                operation("ET", vec![]),
            ])
        }
        "type3_macroman_base" | "type3_macexpert_base" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation(
                "Tj",
                vec![Object::String(
                    vec![if case == "type3_macroman_base" {
                        202
                    } else {
                        33
                    }],
                    lopdf::StringFormat::Literal,
                )],
            ),
            operation("ET", vec![]),
        ]),
        "type3_notdef" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation(
                "Tj",
                vec![Object::String(vec![32], lopdf::StringFormat::Literal)],
            ),
            operation("ET", vec![]),
        ]),
        "composite_cidset_real_program" | "composite_cidset_indirect_basefont_real_program" => {
            content(vec![
                operation("BT", vec![]),
                operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
                operation(
                    "Tj",
                    vec![Object::String(vec![0, 32], lopdf::StringFormat::Literal)],
                ),
                operation("ET", vec![]),
            ])
        }
        "composite_cmap_unknown_usecmap" | "composite_cmap_dictionary_unknown_usecmap" => {
            content(vec![
                operation("BT", vec![]),
                operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
                operation("Tr", vec![3.into()]),
                operation(
                    "Tj",
                    vec![Object::String(vec![32], lopdf::StringFormat::Literal)],
                ),
                operation("ET", vec![]),
            ])
        }
        "composite_cidset_nonidentity_real_program" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation(
                "Tj",
                vec![Object::String(vec![32], lopdf::StringFormat::Literal)],
            ),
            operation("ET", vec![]),
        ]),
        "composite_named_cmap_cidset_real_program" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation(
                "Tj",
                vec![Object::String(vec![0, 0x3f], lopdf::StringFormat::Literal)],
            ),
            operation("ET", vec![]),
        ]),
        "composite_nonidentity_missing_glyph" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation(
                "Tj",
                vec![Object::String(vec![32], lopdf::StringFormat::Literal)],
            ),
            operation("ET", vec![]),
        ]),
        "composite_nonidentity_multibyte_missing_glyph" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation(
                "Tj",
                vec![Object::String(vec![0, 32], lopdf::StringFormat::Literal)],
            ),
            operation("ET", vec![]),
        ]),
        "composite_identity_usecmap_missing_glyph" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation(
                "Tj",
                vec![Object::String(vec![0, 32], lopdf::StringFormat::Literal)],
            ),
            operation("ET", vec![]),
        ]),
        "composite_identity_missing_glyph"
        | "composite_identity_width_mismatch"
        | "composite_identity_width_override_mismatch"
        | "composite_descendant_subtype_indirect_width_mismatch"
        | "composite_dw_indirect_mismatch"
        | "composite_w_singles_element_indirect_mismatch"
        | "composite_stream_cidmap_missing_glyph"
        | "composite_cff_missing_glyph"
        | "composite_cff_present_glyph"
        | "composite_cff_width_mismatch"
        | "composite_cff_cidset_missing" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation(
                "Tj",
                vec![Object::String(vec![0, 32], lopdf::StringFormat::Literal)],
            ),
            operation("ET", vec![]),
        ]),
        "unused_resource" | "unused_invalid_font" | "unicode_name_basefont_unused" => Vec::new(),
        "selected_not_shown" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation("ET", vec![]),
        ]),
        "unembedded_invisible" => text_content(3),
        "mixed_rendering_modes" | "mixed_visible_first" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation(
                "Tr",
                vec![if case == "mixed_visible_first" {
                    0.into()
                } else {
                    3.into()
                }],
            ),
            operation("Tj", vec![Object::string_literal(" ")]),
            operation(
                "Tr",
                vec![if case == "mixed_visible_first" {
                    3.into()
                } else {
                    0.into()
                }],
            ),
            operation("Tj", vec![Object::string_literal(" ")]),
            operation("ET", vec![]),
        ]),
        "graphics_state_visible" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation("Tr", vec![3.into()]),
            operation("q", vec![]),
            operation("Tr", vec![0.into()]),
            operation("Tj", vec![Object::string_literal(" ")]),
            operation("Q", vec![]),
            operation("ET", vec![]),
        ]),
        "graphics_state_invisible" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation("Tr", vec![3.into()]),
            operation("q", vec![]),
            operation("Tr", vec![0.into()]),
            operation("Q", vec![]),
            operation("Tj", vec![Object::string_literal(" ")]),
            operation("ET", vec![]),
        ]),
        "repeated_aliases" | "two_unembedded_fonts" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation("Tj", vec![Object::string_literal(" ")]),
            operation("Tf", vec![Object::Name(b"F2".to_vec()), 12.into()]),
            operation("Tj", vec![Object::string_literal(" ")]),
            operation("ET", vec![]),
        ]),
        "form_unembedded" | "form_unembedded_indirect_subtype" | "nested_form_unembedded" => {
            let form_subtype: Object = if case == "form_unembedded_indirect_subtype" {
                // The Form XObject's own /Subtype as an *indirect*
                // reference to the name /Form, not a direct one --
                // discriminates whether an unresolved indirect /Subtype
                // wrongly makes the whole form (and every font it uses)
                // invisible to font discovery.
                Object::Reference(document.add_object(Object::Name(b"Form".to_vec())))
            } else {
                Object::Name(b"Form".to_vec())
            };
            let form = Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => form_subtype,
                    "BBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
                    "Resources" => resources.clone(),
                },
                text_content(0),
            );
            let mut form_id = document.add_object(form);
            if case == "nested_form_unembedded" {
                form_id = document.add_object(Stream::new(
                    dictionary! {
                        "Type" => "XObject",
                        "Subtype" => "Form",
                        "BBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
                        "Resources" => dictionary! {
                            "XObject" => dictionary! {
                                "Inner" => form_id,
                            },
                        },
                    },
                    content(vec![operation("Do", vec![Object::Name(b"Inner".to_vec())])]),
                ));
            }
            resources = dictionary! {
                "XObject" => dictionary! {
                    "Fm1" => form_id,
                },
            };
            content(vec![operation("Do", vec![Object::Name(b"Fm1".to_vec())])])
        }
        "cyclic_form" => {
            let form_id = document.new_object_id();
            document.objects.insert(
                form_id,
                Object::Stream(Stream::new(
                    dictionary! {
                        "Type" => "XObject",
                        "Subtype" => "Form",
                        "BBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
                        "Resources" => dictionary! {
                            "Font" => font_resources.clone(),
                            "XObject" => dictionary! {
                                "Self" => form_id,
                            },
                        },
                    },
                    content(vec![
                        operation("BT", vec![]),
                        operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
                        operation("Tj", vec![Object::string_literal(" ")]),
                        operation("ET", vec![]),
                        operation("Do", vec![Object::Name(b"Self".to_vec())]),
                    ]),
                )),
            );
            resources = dictionary! {
                "XObject" => dictionary! {
                    "Fm1" => form_id,
                },
            };
            content(vec![operation("Do", vec![Object::Name(b"Fm1".to_vec())])])
        }
        "large_content" => {
            let mut bytes = vec![b' '; 4096];
            bytes.extend_from_slice(&text_content(0));
            bytes
        }
        "deep_graphics_state" => {
            let mut operations = Vec::new();
            operations.extend((0..5).map(|_| operation("q", vec![])));
            operations.extend((0..5).map(|_| operation("Q", vec![])));
            content(operations)
        }
        "tt_glyph_missing" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation("Tr", vec![0.into()]),
            operation("Tj", vec![Object::string_literal("!")]),
            operation("ET", vec![]),
        ]),
        "type1_glyph_missing" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation("Tj", vec![Object::string_literal("!")]),
            operation("ET", vec![]),
        ]),
        "type1_difference_glyph" | "type1_indirect_difference_code" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation("Tj", vec![Object::string_literal("!")]),
            operation("ET", vec![]),
        ]),
        "type1_subset_charset_difference_incomplete" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation("Tj", vec![Object::string_literal("!")]),
            operation("ET", vec![]),
        ]),
        "type1_glyph_present" => text_content(0),
        "type1_subset_charset_incomplete" | "type1_subset_charset_incomplete_indirect_basefont" => {
            text_content(0)
        }
        "tt_nonascii_winansi" | "tt_nonascii_winansi_width_mismatch" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation("Tr", vec![0.into()]),
            operation(
                "Tj",
                vec![Object::String(vec![0xe9], StringFormat::Literal)],
            ),
            operation("ET", vec![]),
        ]),
        "unicode_pua_missing_actual_text" => text_content(0),
        "unicode_pua_with_actual_text" => content(vec![
            operation(
                "BDC",
                vec![
                    Object::Name(b"Span".to_vec()),
                    Object::Dictionary(
                        dictionary! { "ActualText" => Object::string_literal("replacement") },
                    ),
                ],
            ),
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation("Tj", vec![Object::string_literal(" ")]),
            operation("ET", vec![]),
            operation("EMC", vec![]),
        ]),
        "unicode_pua_null_actual_text" => content(vec![
            operation(
                "BDC",
                vec![
                    Object::Name(b"Span".to_vec()),
                    Object::Dictionary(dictionary! { "ActualText" => Object::Null }),
                ],
            ),
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation("Tj", vec![Object::string_literal(" ")]),
            operation("ET", vec![]),
            operation("EMC", vec![]),
        ]),
        "unicode_pua_integer_actual_text" => content(vec![
            operation(
                "BDC",
                vec![
                    Object::Name(b"Span".to_vec()),
                    Object::Dictionary(dictionary! { "ActualText" => 1 }),
                ],
            ),
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation("Tj", vec![Object::string_literal(" ")]),
            operation("ET", vec![]),
            operation("EMC", vec![]),
        ]),
        "unicode_pua_indirect_actual_text" => {
            let actual_text = document.add_object(Object::string_literal("replacement"));
            content(vec![
                operation(
                    "BDC",
                    vec![
                        Object::Name(b"Span".to_vec()),
                        Object::Dictionary(dictionary! { "ActualText" => actual_text }),
                    ],
                ),
                operation("BT", vec![]),
                operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
                operation("Tj", vec![Object::string_literal(" ")]),
                operation("ET", vec![]),
                operation("EMC", vec![]),
            ])
        }
        "unicode_pua_named_actual_text" => content(vec![
            operation(
                "BDC",
                vec![Object::Name(b"Span".to_vec()), Object::Name(b"P".to_vec())],
            ),
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation("Tj", vec![Object::string_literal(" ")]),
            operation("ET", vec![]),
            operation("EMC", vec![]),
        ]),
        "unicode_pua_invisible_missing_actual_text" => content(vec![
            operation("BT", vec![]),
            operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            operation("Tr", vec![3.into()]),
            operation("Tj", vec![Object::string_literal(" ")]),
            operation("ET", vec![]),
        ]),
        _ => text_content(0),
    };
    let contents_id = document.add_object(Stream::new(Dictionary::new(), page_content));
    let mut page = dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => resources,
        "Contents" => contents_id,
    };
    let inherited_resources = if case == "inherited_resources" {
        page.remove(b"Resources")
    } else {
        None
    };
    let page_id = document.add_object(page);
    let mut pages = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![Object::Reference(page_id)],
        "Count" => 1,
    };
    if let Some(resources) = inherited_resources {
        pages.set("Resources", resources);
    }
    document.objects.insert(pages_id, Object::Dictionary(pages));
    let metadata_id = standard_metadata_stream(&mut document);
    let output_intents = single_profile_intent(
        &mut document,
        icc_header(*b"mntr", *b"GRAY", 2, 1),
        Some("GTS_PDFA1"),
    );
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "OutputIntents" => output_intents.expect("output intent"),
    });
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).expect("save font fixture");
    bytes
}

/// A minimal, always-non-embedded `Type1`/Helvetica font: using it anywhere
/// visible must trigger `PDFA1B-FONT-EMBEDDING-001` on its own.
pub fn non_embedded_helper_font(document: &mut Document) -> ObjectId {
    document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
        "FirstChar" => 32,
        "LastChar" => 32,
        "Widths" => vec![278.into()],
    })
}

/// Builds a minimal PDF/A-1b page exercising font-use discovery through a
/// content source other than the page's own content stream or an invoked
/// Form XObject: an annotation appearance stream (including a button
/// Widget's non-selected state), a Pattern's own content, or a Type3 glyph
/// CharProc. Each was confirmed live against veraPDF 1.30.2 to still
/// populate a `PDFont` object for a font used only there, so it must still
/// be checked for embedding (see `content_support::ContentExecutor`'s shared
/// appearance, Pattern, and Type3 execution paths).
pub fn font_content_source_fixture(case: &str) -> Vec<u8> {
    let mut document = pdf_document();
    let pages_id = document.new_object_id();

    let mut resources = Dictionary::new();
    let mut page_content = Vec::new();
    let mut annotations = Vec::new();

    match case {
        // A trivial fill with no font anywhere: every other case below also
        // paints something (a glyph or a filled rectangle), which uses the
        // default DeviceGray colour space and fails PDFA1B-DEVICE-GRAY-001
        // on its own (this fixture's output intent carries no ICC profile).
        // Painting here too keeps that failure common to every case so the
        // rule-ID delta isolates PDFA1B-FONT-EMBEDDING-001 alone.
        "baseline" => {
            page_content = content(vec![
                operation("re", vec![0.into(), 0.into(), 1.into(), 1.into()]),
                operation("f", vec![]),
            ]);
        }
        "annotation_appearance_unembedded" => {
            let helper_font = non_embedded_helper_font(&mut document);
            let appearance_content = content(vec![
                operation("BT", vec![]),
                operation("Tf", vec![Object::Name(b"FAnnot".to_vec()), 8.into()]),
                operation("Tj", vec![Object::string_literal(" ")]),
                operation("ET", vec![]),
            ]);
            let appearance_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 20.into(), 20.into()],
                    "Resources" => dictionary! {
                        "Font" => dictionary! { "FAnnot" => helper_font },
                    },
                },
                appearance_content,
            ));
            annotations.push(document.add_object(dictionary! {
                "Type" => "Annot",
                "Subtype" => "FreeText",
                "Rect" => vec![10.into(), 10.into(), 110.into(), 30.into()],
                "F" => 4,
                "AP" => dictionary! { "N" => appearance_id },
            }));
        }
        "down_appearance_unembedded" => {
            let helper_font = non_embedded_helper_font(&mut document);
            let normal_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 20.into(), 20.into()],
                },
                Vec::new(),
            ));
            let down_content = content(vec![
                operation("BT", vec![]),
                operation("Tf", vec![Object::Name(b"FAnnot".to_vec()), 8.into()]),
                operation("Tj", vec![Object::string_literal(" ")]),
                operation("ET", vec![]),
            ]);
            let down_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 20.into(), 20.into()],
                    "Resources" => dictionary! {
                        "Font" => dictionary! { "FAnnot" => helper_font },
                    },
                },
                down_content,
            ));
            // /D's mere presence already fails PDFA1B-ANNOTATION-AP-ENTRIES-001
            // on its own (confirmed live: a compliant /AP has only /N), but
            // veraPDF 1.30.2 still walks /D for font use regardless, so the
            // unembedded font used only there is independently flagged too.
            annotations.push(document.add_object(dictionary! {
                "Type" => "Annot",
                "Subtype" => "FreeText",
                "Rect" => vec![10.into(), 10.into(), 110.into(), 30.into()],
                "F" => 4,
                "AP" => dictionary! {
                    "N" => normal_id,
                    "D" => down_id,
                },
            }));
        }
        "widget_state_unembedded" => {
            let helper_font = non_embedded_helper_font(&mut document);
            let off_content = content(vec![
                operation("BT", vec![]),
                operation("Tf", vec![Object::Name(b"FAnnot".to_vec()), 8.into()]),
                operation("Tj", vec![Object::string_literal(" ")]),
                operation("ET", vec![]),
            ]);
            let off_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 20.into(), 20.into()],
                    "Resources" => dictionary! {
                        "Font" => dictionary! { "FAnnot" => helper_font },
                    },
                },
                off_content,
            ));
            let yes_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => vec![0.into(), 0.into(), 20.into(), 20.into()],
                    "Resources" => dictionary! {},
                },
                Vec::new(),
            ));
            annotations.push(document.add_object(dictionary! {
                "Type" => "Annot",
                "Subtype" => "Widget",
                "FT" => "Btn",
                "Rect" => vec![10.into(), 10.into(), 30.into(), 30.into()],
                "F" => 4,
                "AS" => "Yes",
                "AP" => dictionary! {
                    "N" => dictionary! {
                        "Off" => off_id,
                        "Yes" => yes_id,
                    },
                },
            }));
        }
        "pattern_unembedded" | "pattern_unused" => {
            let helper_font = non_embedded_helper_font(&mut document);
            let pattern_content = content(vec![
                operation("BT", vec![]),
                operation("Tf", vec![Object::Name(b"FPat".to_vec()), 8.into()]),
                operation("Tj", vec![Object::string_literal(" ")]),
                operation("ET", vec![]),
            ]);
            let pattern_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "Pattern",
                    "PatternType" => 1,
                    "PaintType" => 1,
                    "TilingType" => 1,
                    "BBox" => vec![0.into(), 0.into(), 10.into(), 10.into()],
                    "XStep" => 10,
                    "YStep" => 10,
                    "Resources" => dictionary! {
                        "Font" => dictionary! { "FPat" => helper_font },
                    },
                },
                pattern_content,
            ));
            resources.set("Pattern", dictionary! { "P1" => pattern_id });
            resources.set(
                "ColorSpace",
                dictionary! { "CSP1" => vec![Object::Name(b"Pattern".to_vec())] },
            );
            page_content = if case == "pattern_unembedded" {
                content(vec![
                    operation("re", vec![0.into(), 0.into(), 1.into(), 1.into()]),
                    operation("f", vec![]),
                    operation("q", vec![]),
                    operation("cs", vec![Object::Name(b"CSP1".to_vec())]),
                    operation("scn", vec![Object::Name(b"P1".to_vec())]),
                    operation("re", vec![0.into(), 0.into(), 50.into(), 50.into()]),
                    operation("f", vec![]),
                    operation("Q", vec![]),
                ])
            } else {
                // Same DeviceGray-triggering fill as the shared baseline,
                // but never selecting the Pattern colour space -- the
                // Pattern above is declared but genuinely unused.
                content(vec![
                    operation("re", vec![0.into(), 0.into(), 1.into(), 1.into()]),
                    operation("f", vec![]),
                ])
            };
        }
        "type3_charproc_unembedded" => {
            let helper_font = non_embedded_helper_font(&mut document);
            let charproc_content = content(vec![
                operation(
                    "d1",
                    vec![
                        1000.into(),
                        0.into(),
                        0.into(),
                        0.into(),
                        1000.into(),
                        1000.into(),
                    ],
                ),
                operation("BT", vec![]),
                operation("Tf", vec![Object::Name(b"FInner".to_vec()), 8.into()]),
                operation("Tj", vec![Object::string_literal(" ")]),
                operation("ET", vec![]),
            ]);
            let charproc_id = document.add_object(Stream::new(
                dictionary! {
                    "Resources" => dictionary! {
                        "Font" => dictionary! { "FInner" => helper_font },
                    },
                },
                charproc_content,
            ));
            let type3_font_id = document.add_object(dictionary! {
                "Type" => "Font",
                "Subtype" => "Type3",
                "FontBBox" => vec![0.into(), 0.into(), 1000.into(), 1000.into()],
                "FontMatrix" => vec![
                    0.001.into(),
                    0.into(),
                    0.into(),
                    0.001.into(),
                    0.into(),
                    0.into(),
                ],
                "CharProcs" => dictionary! { "g1" => charproc_id },
                "Encoding" => dictionary! {
                    "Type" => "Encoding",
                    "Differences" => vec![65.into(), Object::Name(b"g1".to_vec())],
                },
                "FirstChar" => 65,
                "LastChar" => 65,
                "Widths" => vec![1000.into()],
            });
            resources.set("Font", dictionary! { "T3" => type3_font_id });
            page_content = content(vec![
                operation("BT", vec![]),
                operation("Tf", vec![Object::Name(b"T3".to_vec()), 12.into()]),
                operation("Tj", vec![Object::string_literal("A")]),
                operation("ET", vec![]),
            ]);
        }
        other => panic!("unknown font_content_source_fixture case {other}"),
    }

    let contents_id = document.add_object(Stream::new(Dictionary::new(), page_content));
    let mut page = dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => resources,
        "Contents" => contents_id,
    };
    if !annotations.is_empty() {
        page.set(
            "Annots",
            annotations
                .into_iter()
                .map(Object::Reference)
                .collect::<Vec<_>>(),
        );
    }
    let page_id = document.add_object(page);
    wrap_pages(&mut document, pages_id, page_id);
    let metadata_id = standard_metadata_stream(&mut document);
    let output_intents = single_intent(&mut document, None, Some("GTS_PDFA1"));
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "OutputIntents" => output_intents.expect("output intent"),
    });
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save font content source fixture");
    bytes
}

pub fn type0_descendant_dictionary(
    document: &mut Document,
    embedded: bool,
    cid_to_gid_map: bool,
) -> ObjectId {
    let descriptor = font_descriptor(document, embedded);
    let descriptor_id = document.add_object(descriptor);
    let mut descendant = dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType2",
        "BaseFont" => "MaiTestFont",
        "CIDSystemInfo" => dictionary! {
            "Registry" => Object::string_literal("Adobe"),
            "Ordering" => Object::string_literal("Identity"),
            "Supplement" => 0,
        },
        "FontDescriptor" => descriptor_id,
        // Matches sfnt::minimal_truetype()'s glyph 0/1 hmtx advance width,
        // so a compliant descendant also passes the width-consistency check.
        "DW" => 500,
    };
    if cid_to_gid_map {
        descendant.set("CIDToGIDMap", "Identity");
    }
    document.add_object(descendant)
}

/// Builds a Type0 font with one or two `/DescendantFonts` entries. PDF32000
/// 9.7.3 requires exactly one entry, and this was confirmed live against
/// veraPDF 1.30.2: it creates a `PDCIDFont` object, and evaluates every
/// per-font predicate against it (including embedding and `/CIDToGIDMap`),
/// only for `DescendantFonts[0]`. A second entry is invisible to veraPDF's
/// object model entirely, however broken -- so `font_embedding.rs`'s
/// `Scanner::record_font` must not independently flag it either.
pub fn type0_descendant_fixture(case: &str) -> Vec<u8> {
    let mut document = pdf_document();
    let pages_id = document.new_object_id();

    let descendant0 = type0_descendant_dictionary(&mut document, true, true);
    if case == "unicode_type0_gb1" {
        document
            .get_object_mut(descendant0)
            .expect("descendant0")
            .as_dict_mut()
            .expect("descendant0 dictionary")
            .set(
                "CIDSystemInfo",
                dictionary! {
                    "Registry" => Object::string_literal("Adobe"),
                    "Ordering" => Object::string_literal("GB1"),
                    "Supplement" => 4,
                },
            );
    }
    let mut descendants = vec![Object::Reference(descendant0)];
    match case {
        "baseline"
        | "unicode_type0_identity_h"
        | "unicode_type0_identity_v"
        | "unicode_type0_gb1" => {}
        "indirect_identity_cidtogidmap" => {
            // The first descendant's own /CIDToGIDMap as an *indirect*
            // reference to the name /Identity, not a direct one --
            // confirmed live against veraPDF 1.30.2 to resolve rendered
            // CIDs to the same glyphs as a direct /Identity would. /DW is
            // also deliberately mismatched (999, not the real glyph 1
            // advance width of 500): `glyph_for`'s `None` case (an
            // unresolvable map) is a silent `continue`, not a pushed
            // failure, so a *matching*-width case can't distinguish
            // "resolved and correct" from "unresolved and skipped" --
            // only a genuine mismatch proves the map was actually resolved
            // and checked.
            let indirect_identity = document.add_object(Object::Name(b"Identity".to_vec()));
            let descendant0_dict = document
                .get_object_mut(descendant0)
                .expect("descendant0")
                .as_dict_mut()
                .expect("descendant0 dict");
            descendant0_dict.set("CIDToGIDMap", indirect_identity);
            descendant0_dict.set("DW", 999);
        }
        "second_descendant_unembedded" => {
            descendants.push(Object::Reference(type0_descendant_dictionary(
                &mut document,
                false,
                true,
            )));
        }
        "second_descendant_missing_cidtogidmap" => {
            descendants.push(Object::Reference(type0_descendant_dictionary(
                &mut document,
                true,
                false,
            )));
        }
        other => panic!("unknown type0_descendant_fixture case {other}"),
    }

    let cmap_name = if case == "unicode_type0_identity_v" {
        "Identity-V"
    } else {
        "Identity-H"
    };
    let cmap_content =
        b"/CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> def\n\
/CMapName /Identity-H def\n\
1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n\
1 begincidrange\n<0000> <FFFF> 0\nendcidrange\n"
            .to_vec();
    let cmap_id = document.add_object(Stream::new(
        dictionary! { "Type" => "CMap", "CMapName" => cmap_name },
        cmap_content,
    ));
    let encoding = if matches!(
        case,
        "unicode_type0_identity_h" | "unicode_type0_identity_v"
    ) {
        Object::Name(cmap_name.as_bytes().to_vec())
    } else {
        cmap_id.into()
    };

    let type0_font = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "MaiTestFont",
        "Encoding" => encoding,
        "DescendantFonts" => descendants,
    });

    let resources = dictionary! {
        "Font" => dictionary! { "T0" => type0_font },
    };
    // CID 0 is deliberately skipped by `inspect_rendered_cidfont_glyphs`
    // (`.notdef`), so a case that means to exercise `CIDToGIDMap`
    // resolution (`glyph_for`) must render a non-zero CID instead.
    let rendered_cid = if case == "indirect_identity_cidtogidmap" {
        vec![0, 1]
    } else {
        vec![0, 0]
    };
    let page_content = content(vec![
        operation("BT", vec![]),
        operation("Tf", vec![Object::Name(b"T0".to_vec()), 12.into()]),
        operation(
            "Tj",
            vec![Object::String(rendered_cid, StringFormat::Literal)],
        ),
        operation("ET", vec![]),
    ]);
    let contents_id = document.add_object(Stream::new(Dictionary::new(), page_content));
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => resources,
        "Contents" => contents_id,
    });
    wrap_pages(&mut document, pages_id, page_id);
    let metadata_id = standard_metadata_stream(&mut document);
    let output_intents = single_intent(&mut document, None, Some("GTS_PDFA1"));
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "OutputIntents" => output_intents.expect("output intent"),
    });
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save Type0 descendant fixture");
    bytes
}

/// A symbolic TrueType font whose embedded program's `cmap` table is fully
/// valid (declaring 2 subtables) but whose `maxp` table is truncated to 2
/// bytes (too short to read `numGlyphs`), so `ttf_parser`'s whole-font
/// `Face::parse` fails even though the `cmap` table itself is perfectly
/// readable. Confirmed live against veraPDF 1.30.2: it reads the `cmap`
/// table's subtable count directly (matching its own mapping note, "read
/// from the bounded SFNT cmap table header"), so `PDFA1B-TRUETYPE-SYMBOLIC-
/// CMAP-001` must not gate on a full-font parse either -- see
/// `truetype_cmap_count` in `font_embedding.rs`, which now uses
/// `ttf_parser::RawFace` to read just the `cmap` table directly.
pub fn symbolic_cmap_with_malformed_maxp_fixture() -> Vec<u8> {
    let mut document = pdf_document();
    let pages_id = document.new_object_id();

    let mut head = vec![0; 54];
    head.get_mut(0..4)
        .expect("TrueType head version")
        .copy_from_slice(&0x0001_0000u32.to_be_bytes());
    head.get_mut(4..8)
        .expect("TrueType head revision")
        .copy_from_slice(&0x0001_0000u32.to_be_bytes());
    head.get_mut(12..16)
        .expect("TrueType head checksum")
        .copy_from_slice(&0x5F0F_3CF5u32.to_be_bytes());
    head.get_mut(18..20)
        .expect("TrueType head units-per-em")
        .copy_from_slice(&1000u16.to_be_bytes());
    head.get_mut(46..48)
        .expect("TrueType head index-to-loc-format")
        .copy_from_slice(&8u16.to_be_bytes());

    let malformed_maxp = vec![0u8; 2];

    let cmap_header_length = 4 + 2 * 8;
    let mut cmap = vec![0u8; cmap_header_length + 262];
    cmap.get_mut(2..4)
        .expect("cmap subtable count")
        .copy_from_slice(&2u16.to_be_bytes());
    for index in 0..2usize {
        let record = 4 + index * 8;
        cmap.get_mut(record..record + 2)
            .expect("cmap platform identifier")
            .copy_from_slice(&3u16.to_be_bytes());
        cmap.get_mut(record + 2..record + 4)
            .expect("cmap encoding identifier")
            .copy_from_slice(
                &u16::try_from(index + 1)
                    .expect("small cmap encoding identifier")
                    .to_be_bytes(),
            );
        cmap.get_mut(record + 4..record + 8)
            .expect("cmap subtable offset")
            .copy_from_slice(
                &u32::try_from(cmap_header_length)
                    .expect("small cmap header")
                    .to_be_bytes(),
            );
    }
    cmap.get_mut(cmap_header_length..cmap_header_length + 2)
        .expect("cmap format")
        .copy_from_slice(&0u16.to_be_bytes());
    cmap.get_mut(cmap_header_length + 2..cmap_header_length + 4)
        .expect("cmap subtable length")
        .copy_from_slice(&262u16.to_be_bytes());
    *cmap
        .get_mut(cmap_header_length + 6 + 65)
        .expect("cmap glyph slot") = 1;

    let tables = vec![
        (*b"OS/2", vec![0u8; 78]),
        (*b"cmap", cmap),
        (*b"glyf", vec![0u8; 4]),
        (*b"head", head),
        (*b"hhea", vec![0u8; 36]),
        (*b"hmtx", vec![0u8; 8]),
        (*b"loca", vec![0u8; 4]),
        (*b"maxp", malformed_maxp),
        (*b"name", vec![0u8; 6]),
        (*b"post", vec![0u8; 32]),
    ];
    let font_program = sfnt::build_sfnt(tables);

    let font_file = document.add_object(Stream::new(Dictionary::new(), font_program));
    let descriptor = document.add_object(dictionary! {
        "Type" => "FontDescriptor",
        "FontName" => "MaiTestFont",
        "Flags" => 4,
        "FontBBox" => vec![0.into(), 0.into(), 1000.into(), 1000.into()],
        "ItalicAngle" => 0,
        "Ascent" => 800,
        "Descent" => -200,
        "CapHeight" => 700,
        "StemV" => 80,
        "FontFile2" => font_file,
    });
    let font_id = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "TrueType",
        "BaseFont" => "MaiTestFont",
        "FirstChar" => 65,
        "LastChar" => 65,
        "Widths" => vec![Object::Integer(1000)],
        "FontDescriptor" => descriptor,
    });

    let resources = dictionary! {
        "Font" => dictionary! { "FSym" => font_id },
    };
    let page_content = content(vec![
        operation("BT", vec![]),
        operation("Tf", vec![Object::Name(b"FSym".to_vec()), 12.into()]),
        operation("Tj", vec![Object::String(vec![65], StringFormat::Literal)]),
        operation("ET", vec![]),
    ]);
    let contents_id = document.add_object(Stream::new(Dictionary::new(), page_content));
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => resources,
        "Contents" => contents_id,
    });
    wrap_pages(&mut document, pages_id, page_id);
    let metadata_id = standard_metadata_stream(&mut document);
    let output_intents = single_intent(&mut document, None, Some("GTS_PDFA1"));
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "Metadata" => metadata_id,
        "OutputIntents" => output_intents.expect("output intent"),
    });
    let info_id = document.add_object(complete_info());
    document.trailer.set("Root", catalog_id);
    document.trailer.set("Info", info_id);
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .expect("save malformed-maxp experiment fixture");
    bytes
}

pub fn font_descriptor(document: &mut Document, embedded: bool) -> Dictionary {
    let mut descriptor = dictionary! {
        "Type" => "FontDescriptor",
        "FontName" => "MaiTestFont",
        "Flags" => 32,
        "FontBBox" => vec![0.into(), 0.into(), 1000.into(), 1000.into()],
        "ItalicAngle" => 0,
        "Ascent" => 800,
        "Descent" => -200,
        "CapHeight" => 700,
        "StemV" => 80,
    };
    if embedded {
        let font_file =
            document.add_object(Stream::new(Dictionary::new(), sfnt::minimal_truetype()));
        descriptor.set("FontFile2", font_file);
    }
    descriptor
}

pub fn type1_program(char_names: &[&str]) -> Vec<u8> {
    let char_strings = char_names
        .iter()
        .map(|name| format!("/{name} 1 RD"))
        .collect::<String>();
    let plaintext = [
        vec![0; 4],
        format!("dup /Private 1 dict dup begin /CharStrings 1 dict dup begin {char_strings}")
            .into_bytes(),
    ]
    .concat();
    let mut state = 55_665_u16;
    let encrypted = plaintext
        .into_iter()
        .map(|plaintext| {
            let ciphertext = plaintext ^ (state >> 8) as u8;
            state = state
                .wrapping_add(u16::from(ciphertext))
                .wrapping_mul(52_845)
                .wrapping_add(22_719);
            ciphertext
        })
        .collect::<Vec<_>>();
    [
        b"%!PS-AdobeFont\n/Encoding StandardEncoding def\neexec\n".as_slice(),
        encrypted.as_slice(),
    ]
    .concat()
}

pub fn pdf_type1_program(bytes: &[u8]) -> (Vec<u8>, usize, usize, usize) {
    if !bytes.starts_with(&[0x80, 0x01]) {
        let length1 = bytes
            .windows(b"eexec\n".len())
            .position(|window| window == b"eexec\n")
            .map(|position| position + b"eexec\n".len())
            .unwrap_or(bytes.len());
        let Some(clear_to_mark) = bytes
            .windows(b"cleartomark".len())
            .position(|window| window == b"cleartomark")
        else {
            return (bytes.to_vec(), length1, bytes.len() - length1, 0);
        };
        let mut encrypted_end = clear_to_mark;
        while encrypted_end > length1
            && matches!(
                *bytes
                    .get(encrypted_end - 1)
                    .expect("Type1 encrypted trailer byte"),
                b'0' | b' ' | b'\t' | b'\r' | b'\n'
            )
        {
            encrypted_end -= 1;
        }
        let hex = bytes
            .get(length1..encrypted_end)
            .expect("Type1 encrypted range")
            .iter()
            .copied()
            .filter(|byte| !byte.is_ascii_whitespace())
            .collect::<Vec<_>>();
        if !hex.is_empty() && hex.len() % 2 == 0 && hex.iter().all(u8::is_ascii_hexdigit) {
            let encrypted = hex
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| {
                    let digit = |byte| match byte {
                        b'0'..=b'9' => byte - b'0',
                        b'a'..=b'f' => byte - b'a' + 10,
                        b'A'..=b'F' => byte - b'A' + 10,
                        _ => panic!("invalid Type1 hexadecimal byte"),
                    };
                    let &[first, second] = pair;
                    digit(first) << 4 | digit(second)
                })
                .collect::<Vec<_>>();
            let trailer = bytes.get(encrypted_end..).expect("Type1 trailer");
            let mut payload = Vec::with_capacity(length1 + encrypted.len() + trailer.len());
            payload.extend_from_slice(bytes.get(..length1).expect("Type1 clear section"));
            payload.extend_from_slice(&encrypted);
            payload.extend_from_slice(trailer);
            let encrypted_length = encrypted.len();
            let trailer_length = trailer.len();
            return (payload, length1, encrypted_length, trailer_length);
        }
        return (bytes.to_vec(), length1, bytes.len() - length1, 0);
    }
    let mut payload = Vec::new();
    let mut lengths = [0usize; 3];
    let mut position = 0usize;
    let mut segment = 0usize;
    while bytes.get(position) == Some(&0x80) && segment < 3 {
        let Some(kind) = bytes.get(position + 1).copied() else {
            break;
        };
        if kind == 3 {
            break;
        }
        let Some(length) = bytes
            .get(position + 2..position + 6)
            .and_then(|value| value.try_into().ok())
            .map(u32::from_le_bytes)
            .and_then(|value| usize::try_from(value).ok())
        else {
            break;
        };
        let start = position + 6;
        let Some(end) = start.checked_add(length) else {
            break;
        };
        let Some(data) = bytes.get(start..end) else {
            break;
        };
        payload.extend_from_slice(data);
        *lengths.get_mut(segment).expect("Type1 segment slot") = length;
        position = end;
        segment += 1;
    }
    (
        payload,
        *lengths.first().expect("Type1 length 1"),
        *lengths.get(1).expect("Type1 length 2"),
        *lengths.get(2).expect("Type1 length 3"),
    )
}

pub fn type1_program_with_width(width: u16) -> Vec<u8> {
    let encode_number = |value: u16| -> Vec<u8> {
        if value <= 107 {
            vec![(value + 139) as u8]
        } else {
            let value = value - 108;
            vec![(247 + value / 256) as u8, (value % 256) as u8]
        }
    };
    let mut charstring = vec![0, 0, 0, 0];
    charstring.extend(encode_number(0));
    charstring.extend(encode_number(width));
    charstring.extend([13, 14]); // hsbw, endchar
    let mut state = 4_330_u16;
    let encrypted_charstring = charstring
        .into_iter()
        .map(|plaintext| {
            let ciphertext = plaintext ^ (state >> 8) as u8;
            state = state
                .wrapping_add(u16::from(ciphertext))
                .wrapping_mul(52_845)
                .wrapping_add(22_719);
            ciphertext
        })
        .collect::<Vec<_>>();
    let mut plaintext = vec![0, 0, 0, 0];
    plaintext.extend_from_slice(
        format!(
            "dup /Private 2 dict dup begin /lenIV 4 def /CharStrings 1 dict dup begin /space {} RD ",
            encrypted_charstring.len()
        )
        .as_bytes(),
    );
    plaintext.extend_from_slice(&encrypted_charstring);
    plaintext.extend_from_slice(b" ND end end");
    let mut state = 55_665_u16;
    let encrypted = plaintext
        .into_iter()
        .map(|plaintext| {
            let ciphertext = plaintext ^ (state >> 8) as u8;
            state = state
                .wrapping_add(u16::from(ciphertext))
                .wrapping_mul(52_845)
                .wrapping_add(22_719);
            ciphertext
        })
        .collect::<Vec<_>>();
    [
        b"%!PS-AdobeFont\n/Encoding StandardEncoding def\neexec\n".as_slice(),
        encrypted.as_slice(),
    ]
    .concat()
}

pub fn text_content(rendering_mode: i64) -> Vec<u8> {
    content(vec![
        operation("BT", vec![]),
        operation("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
        operation("Tr", vec![rendering_mode.into()]),
        operation("Tj", vec![Object::string_literal(" ")]),
        operation("ET", vec![]),
    ])
}

pub fn embedded_cmap(ordering: &str, wmode: i64, cid_start: u32) -> Vec<u8> {
    format!(
        "/CIDInit /ProcSet findresource begin\n\
         12 dict begin\n\
         begincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering ({ordering}) /Supplement 0 >> def\n\
         /CMapName /Page-CMap def\n\
         /CMapType 1 def\n\
         /WMode {wmode} def\n\
         1 begincodespacerange\n\
         <00> <FF>\n\
         endcodespacerange\n\
         1 begincidrange\n\
         <00> <FF> {cid_start}\n\
         endcidrange\n\
         endcmap\n\
         CMapName currentdict /CMap defineresource pop\n\
         end\n\
         end\n"
    )
    .into_bytes()
}

/// A raw CFF1 program with `.notdef` and, optionally, the standard `space`
/// glyph. Both charstrings are intentionally minimal but valid endchar-only
/// programs; the standard charset maps glyph ID one to `space`.
pub fn minimal_type1c(with_space: bool) -> Vec<u8> {
    let glyphs = usize::from(with_space) + 1;
    let mut bytes = vec![1, 0, 4, 0]; // header
    bytes.extend_from_slice(&0_u16.to_be_bytes()); // Name INDEX
    bytes.extend_from_slice(&1_u16.to_be_bytes()); // Top DICT INDEX count
    bytes.extend_from_slice(&[1, 1, 5]); // offset size, offsets
    let charstrings_offset = 19usize;
    let charset_offset = charstrings_offset + 4 + glyphs * 3;
    bytes.extend_from_slice(&[
        (charstrings_offset + 139) as u8,
        17, // CharStrings operator
        (charset_offset + 139) as u8,
        15, // charset operator
    ]);
    bytes.extend_from_slice(&0_u16.to_be_bytes()); // String INDEX
    bytes.extend_from_slice(&0_u16.to_be_bytes()); // Global Subrs INDEX
    bytes.extend_from_slice(&(glyphs as u16).to_be_bytes());
    bytes.push(1); // CharStrings INDEX offset size
    for offset in 0..=glyphs {
        bytes.push((offset * 2 + 1) as u8);
    }
    for _ in 0..glyphs {
        bytes.extend_from_slice(&[139, 14]); // zero width then endchar
    }
    bytes.push(0); // charset format 0
    if with_space {
        bytes.extend_from_slice(&1_u16.to_be_bytes()); // SID 1 = space
    }
    bytes
}

/// A raw CFF1 program that relies on the default ISOAdobe charset, with
/// `.notdef` and, optionally, the standard `space` glyph.
pub fn minimal_type1c_default_charset(with_space: bool) -> Vec<u8> {
    let glyphs = usize::from(with_space) + 1;
    let charstrings_offset = 17usize;
    let mut bytes = vec![1, 0, 4, 0]; // header
    bytes.extend_from_slice(&0_u16.to_be_bytes()); // Name INDEX
    bytes.extend_from_slice(&1_u16.to_be_bytes()); // Top DICT INDEX count
    bytes.extend_from_slice(&[1, 1, 3]); // offset size, offsets
    bytes.extend_from_slice(&[
        (charstrings_offset + 139) as u8,
        17, // CharStrings operator
    ]);
    bytes.extend_from_slice(&0_u16.to_be_bytes()); // String INDEX
    bytes.extend_from_slice(&0_u16.to_be_bytes()); // Global Subrs INDEX
    bytes.extend_from_slice(&(glyphs as u16).to_be_bytes());
    bytes.push(1); // CharStrings INDEX offset size
    for offset in 0..=glyphs {
        bytes.push((offset * 2 + 1) as u8);
    }
    for _ in 0..glyphs {
        bytes.extend_from_slice(&[139, 14]); // zero width then endchar
    }
    bytes
}

/// A CID-keyed raw CFF1 program with `.notdef` and, optionally, CID 32.
/// CID CFF requires ROS, an explicit charset, FDArray, and FDSelect even when
/// the fixture only needs glyph-to-CID lookup.
pub fn minimal_cidfonttype0c(with_cid_32: bool) -> Vec<u8> {
    let glyphs = usize::from(with_cid_32) + 1;
    let charstrings_offset = 30usize;
    let charstrings_len = 4 + glyphs * 3;
    let charset_offset = charstrings_offset + charstrings_len;
    let charset_len = 1 + (glyphs - 1) * 2;
    let fd_array_offset = charset_offset + charset_len;
    let fd_array_len = 8usize;
    let fd_select_offset = fd_array_offset + fd_array_len;
    let private_offset = fd_select_offset + 1 + glyphs;

    let mut bytes = vec![1, 0, 4, 0]; // header
    bytes.extend_from_slice(&0_u16.to_be_bytes()); // Name INDEX
    bytes.extend_from_slice(&1_u16.to_be_bytes()); // Top DICT INDEX count
    bytes.extend_from_slice(&[1, 1, 16]); // offset size, offsets
    bytes.extend_from_slice(&[
        (charset_offset + 139) as u8,
        15, // charset
        (charstrings_offset + 139) as u8,
        17, // CharStrings
        139,
        139,
        139,
        12,
        30, // ROS
        (fd_array_offset + 139) as u8,
        12,
        36, // FDArray
        (fd_select_offset + 139) as u8,
        12,
        37, // FDSelect
    ]);
    bytes.extend_from_slice(&0_u16.to_be_bytes()); // String INDEX
    bytes.extend_from_slice(&0_u16.to_be_bytes()); // Global Subrs INDEX
    bytes.extend_from_slice(&(glyphs as u16).to_be_bytes());
    bytes.push(1); // CharStrings INDEX offset size
    for offset in 0..=glyphs {
        bytes.push((offset * 2 + 1) as u8);
    }
    for _ in 0..glyphs {
        bytes.extend_from_slice(&[139, 14]);
    }
    bytes.push(0); // charset format 0
    if with_cid_32 {
        bytes.extend_from_slice(&32_u16.to_be_bytes());
    }
    bytes.extend_from_slice(&[
        0,
        1,
        1,
        1,
        4, // one FD dict, offset range 1..4
        141,
        (private_offset + 139) as u8,
        18, // Private size/offset
    ]);
    bytes.push(0); // FDSelect format 0
    bytes.extend(std::iter::repeat_n(0, glyphs));
    bytes.extend_from_slice(&[139, 20, 139, 21]); // defaultWidth and nominalWidth
    bytes
}

pub fn embedded_two_byte_cmap(ordering: &str, wmode: i64, cid_start: u32) -> Vec<u8> {
    format!(
        "/CIDInit /ProcSet findresource begin\n\
         12 dict begin\n\
         begincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering ({ordering}) /Supplement 0 >> def\n\
         /CMapName /Page-CMap def\n\
         /CMapType 1 def\n\
         /WMode {wmode} def\n\
         1 begincodespacerange\n\
         <0000> <FFFF>\n\
         endcodespacerange\n\
         1 begincidrange\n\
         <0000> <FFFF> {cid_start}\n\
         endcidrange\n\
         endcmap\n\
         CMapName currentdict /CMap defineresource pop\n\
         end\n\
         end\n"
    )
    .into_bytes()
}

pub fn embedded_identity_usecmap(ordering: &str, wmode: i64) -> Vec<u8> {
    format!(
        "/CIDInit /ProcSet findresource begin\n\
         12 dict begin\n\
         begincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering ({ordering}) /Supplement 0 >> def\n\
         /CMapName /Page-CMap def\n\
         /CMapType 1 def\n\
         /WMode {wmode} def\n\
         /Identity-H usecmap\n\
         endcmap\n\
         CMapName currentdict /CMap defineresource pop\n\
         end\n\
         end\n"
    )
    .into_bytes()
}

pub fn embedded_unknown_usecmap(ordering: &str, wmode: i64) -> Vec<u8> {
    format!(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering ({ordering}) /Supplement 0 >> def\n\
         /CMapName /Page-CMap def\n/CMapType 1 def\n/WMode {wmode} def\n\
         1 begincodespacerange\n\
         <00> <FF>\n\
         endcodespacerange\n\
         1 begincidrange\n\
         <20> <20> 32\n\
         endcidrange\n\
         /Adobe-Japan1-UCS2 usecmap\nendcmap\nCMapName currentdict /CMap defineresource pop\n\
         end\nend\n"
    )
    .into_bytes()
}

pub fn content(operations: Vec<Operation>) -> Vec<u8> {
    Content { operations }.encode().expect("encode content")
}

pub fn operation(operator: &str, operands: Vec<Object>) -> Operation {
    Operation::new(operator, operands)
}

/// Adds the standard `/Type /Metadata /Subtype /XML` stream carrying
/// `BASE_XMP`. `metadata_fixture`, whose whole point is exercising variant
/// metadata shapes, builds its own instead of calling this.
pub fn standard_metadata_stream(document: &mut Document) -> ObjectId {
    document.add_object(Stream::new(
        dictionary! {
            "Type" => "Metadata",
            "Subtype" => "XML",
        },
        BASE_XMP.to_vec(),
    ))
}

/// Inserts the reserved `pages_id` object as a `/Pages` dictionary with a
/// single `/Kids` entry. Fixtures whose page tree needs anything more (e.g.
/// an inherited `/Resources` entry) build their own `/Pages` dictionary
/// instead of calling this.
pub fn wrap_pages(document: &mut Document, pages_id: ObjectId, page_id: ObjectId) {
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1,
        }),
    );
}

pub fn single_intent(
    document: &mut Document,
    profile: Option<Object>,
    subtype: Option<&str>,
) -> Option<Object> {
    let intent_id = document.add_object(output_intent_dictionary(profile, subtype));
    Some(Object::Array(vec![Object::Reference(intent_id)]))
}

pub fn single_profile_intent(
    document: &mut Document,
    profile: Vec<u8>,
    subtype: Option<&str>,
) -> Option<Object> {
    let profile = profile_reference(document, profile);
    single_intent(document, Some(profile), subtype)
}

pub fn two_intents(document: &mut Document, first: Object, second: Object) -> Option<Object> {
    let first = document.add_object(output_intent_dictionary(Some(first), Some("GTS_PDFA1")));
    let second = document.add_object(output_intent_dictionary(Some(second), Some("GTS_PDFA1")));
    Some(Object::Array(vec![
        Object::Reference(first),
        Object::Reference(second),
    ]))
}

pub fn output_intent_dictionary(profile: Option<Object>, subtype: Option<&str>) -> Dictionary {
    let mut dictionary = dictionary! {
        "Type" => "OutputIntent",
        "OutputConditionIdentifier" => Object::string_literal("Test"),
    };
    if let Some(subtype) = subtype {
        dictionary.set("S", Object::Name(subtype.as_bytes().to_vec()));
    }
    if let Some(profile) = profile {
        dictionary.set("DestOutputProfile", profile);
    }
    dictionary
}

pub fn profile_reference(document: &mut Document, bytes: Vec<u8>) -> Object {
    Object::Reference(document.add_object(profile_stream(bytes)))
}

pub fn compressed_profile_reference(document: &mut Document, bytes: Vec<u8>) -> Object {
    let mut stream = profile_stream(bytes);
    stream.compress().expect("compress ICC test profile");
    Object::Reference(document.add_object(stream))
}

pub fn profile_stream(bytes: Vec<u8>) -> Stream {
    let components = bytes.get(16..20).and_then(|signature| match signature {
        b"GRAY" => Some(1),
        b"RGB " | b"Lab " => Some(3),
        b"CMYK" => Some(4),
        _ => None,
    });
    let mut dictionary = Dictionary::new();
    if let Some(components) = components {
        dictionary.set("N", components);
    }
    Stream::new(dictionary, bytes)
}

pub fn icc_header(
    device_class: [u8; 4],
    color_space: [u8; 4],
    version_major: u8,
    version_minor: u8,
) -> Vec<u8> {
    let mut bytes = vec![0; 20];
    bytes
        .get_mut(0..4)
        .expect("ICC profile size")
        .copy_from_slice(&20u32.to_be_bytes());
    *bytes.get_mut(8).expect("ICC version major") = version_major;
    *bytes.get_mut(9).expect("ICC version minor") = version_minor << 4;
    bytes
        .get_mut(12..16)
        .expect("ICC device class")
        .copy_from_slice(&device_class);
    bytes
        .get_mut(16..20)
        .expect("ICC color space")
        .copy_from_slice(&color_space);
    bytes
}

pub fn complete_info() -> Dictionary {
    dictionary! {
        "Title" => Object::string_literal("Title"),
        "Author" => Object::string_literal("Author"),
        "Subject" => Object::string_literal("Subject"),
        "Keywords" => Object::string_literal("rust,pdf"),
        "Creator" => Object::string_literal("tool"),
        "Producer" => Object::string_literal("producer"),
        "CreationDate" => Object::string_literal("D:20260727123045+02'00'"),
        "ModDate" => Object::string_literal("D:20260727123045+02'00'"),
    }
}

pub enum Occurrence {
    All,
    First,
    Last,
}

pub fn replace(bytes: &mut Vec<u8>, from: &str, to: &str) {
    replace_occurrence(bytes, from, to, Occurrence::All);
}

pub fn replace_first(bytes: &mut Vec<u8>, from: &str, to: &str) {
    replace_occurrence(bytes, from, to, Occurrence::First);
}

pub fn replace_last(bytes: &mut Vec<u8>, from: &str, to: &str) {
    replace_occurrence(bytes, from, to, Occurrence::Last);
}

pub fn replace_occurrence(bytes: &mut Vec<u8>, from: &str, to: &str, occurrence: Occurrence) {
    let text = String::from_utf8(bytes.clone()).expect("XMP fixture is UTF-8");
    let replaced = match occurrence {
        Occurrence::All => {
            assert!(text.contains(from), "fixture does not contain {from:?}");
            text.replace(from, to)
        }
        Occurrence::First => {
            assert!(text.contains(from), "fixture does not contain {from:?}");
            text.replacen(from, to, 1)
        }
        Occurrence::Last => {
            let index = text.rfind(from).expect("fixture occurrence");
            let mut result = text;
            result.replace_range(index..index + from.len(), to);
            result
        }
    };
    *bytes = replaced.into_bytes();
}

pub const BASE_XMP: &[u8] = br#"<?xpacket begin=""?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
 xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/"
 xmlns:dc="http://purl.org/dc/elements/1.1/"
 xmlns:pdf="http://ns.adobe.com/pdf/1.3/"
 xmlns:xmp="http://ns.adobe.com/xap/1.0/">
<rdf:Description pdfaid:part="1" pdfaid:conformance="B"
 pdf:Keywords="rust,pdf" pdf:Producer="producer"
 xmp:CreatorTool="tool" xmp:CreateDate="2026-07-27T12:30:45+02:00"
 xmp:ModifyDate="2026-07-27T12:30:45+02:00">
<dc:title><rdf:Alt><rdf:li xml:lang="fr">Titre</rdf:li>
<rdf:li xml:lang="x-default">Title</rdf:li></rdf:Alt></dc:title>
<dc:creator><rdf:Seq><rdf:li>Author</rdf:li></rdf:Seq></dc:creator>
<dc:description><rdf:Alt><rdf:li xml:lang="x-default">Subject</rdf:li>
</rdf:Alt></dc:description>
</rdf:Description>
</rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#;

pub const EXTENSION_SCHEMA_BLOCK: &str = r#"
<rdf:Description
 xmlns:pdfaExtension="http://www.aiim.org/pdfa/ns/extension/"
 xmlns:pdfaSchema="http://www.aiim.org/pdfa/ns/schema#"
 xmlns:pdfaProperty="http://www.aiim.org/pdfa/ns/property#"
 xmlns:pdfaType="http://www.aiim.org/pdfa/ns/type#"
 xmlns:pdfaField="http://www.aiim.org/pdfa/ns/field#"
 xmlns:extensionAlias="http://www.aiim.org/pdfa/ns/extension/"
 xmlns:schemaAlias="http://www.aiim.org/pdfa/ns/schema#"
 xmlns:propertyAlias="http://www.aiim.org/pdfa/ns/property#"
 xmlns:typeAlias="http://www.aiim.org/pdfa/ns/type#"
 xmlns:fieldAlias="http://www.aiim.org/pdfa/ns/field#">
<pdfaExtension:schemas><rdf:Bag>
<rdf:li rdf:parseType="Resource">
<pdfaSchema:schema>Example schema</pdfaSchema:schema>
<pdfaSchema:namespaceURI>http://example.com/ns/</pdfaSchema:namespaceURI>
<pdfaSchema:prefix>ex</pdfaSchema:prefix>
<pdfaSchema:property><rdf:Seq>
<rdf:li rdf:parseType="Resource">
<pdfaProperty:name>example</pdfaProperty:name>
<pdfaProperty:valueType>Text</pdfaProperty:valueType>
<pdfaProperty:category>external</pdfaProperty:category>
<pdfaProperty:description>Example property</pdfaProperty:description>
</rdf:li>
</rdf:Seq></pdfaSchema:property>
<pdfaSchema:valueType><rdf:Seq>
<rdf:li rdf:parseType="Resource">
<pdfaType:type>CustomType</pdfaType:type>
<pdfaType:namespaceURI>http://example.com/type/</pdfaType:namespaceURI>
<pdfaType:prefix>extype</pdfaType:prefix>
<pdfaType:description>Example type</pdfaType:description>
<pdfaType:field><rdf:Seq>
<rdf:li rdf:parseType="Resource">
<pdfaField:name>member</pdfaField:name>
<pdfaField:valueType>Text</pdfaField:valueType>
<pdfaField:description>Example member</pdfaField:description>
</rdf:li>
</rdf:Seq></pdfaType:field>
</rdf:li>
</rdf:Seq></pdfaSchema:valueType>
</rdf:li>
</rdf:Bag></pdfaExtension:schemas>
</rdf:Description>
"#;
