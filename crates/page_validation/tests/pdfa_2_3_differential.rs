use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::PathBuf;

use page_validation::SafetyLimits;
use page_validation::differential::{
    ComparisonClassification, CoverageGapPolicy, DifferentialRunner, ReferenceConfig,
    ReferenceProfile,
};
use serde::Deserialize;
use serde_json::Value;

pub mod common;

const PROFILE_MANIFEST: &str = "tests/fixtures/verapdf-diff-cases-2-3.json";
const SOURCE_MANIFEST: &str = "tests/fixtures/verapdf-diff-cases.json";

#[derive(Debug, Deserialize)]
struct ProfileManifest {
    reference: ReferenceManifest,
    profiles: Vec<String>,
    mutation_count: usize,
}

#[derive(Debug, Deserialize)]
struct ReferenceManifest {
    product: String,
    version: String,
    source_manifest: String,
}

#[derive(Debug, Deserialize)]
struct SourceManifest {
    checked_in_mutations: Vec<CheckedInMutation>,
}

#[derive(Debug, Deserialize)]
struct CheckedInMutation {
    path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum EvidenceKind {
    Fail,
    Pass,
    Inapplicable,
    ReferenceParserDiscrepancy,
    Unrepresentable,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct AtomicCandidate {
    family: String,
    case: String,
    path: Option<PathBuf>,
    reidentify: bool,
    local_rule_ids: Vec<String>,
    kind: EvidenceKind,
}

#[test]
fn pdfa_2_and_3_profile_mutation_corpus_is_complete_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        eprintln!("VERAPDF_BIN is unset; skipping PDF/A-2/3 differential corpus");
        return;
    };

    let manifest: ProfileManifest = serde_json::from_slice(
        &fs::read(PROFILE_MANIFEST).expect("read PDF/A-2/3 differential manifest"),
    )
    .expect("parse PDF/A-2/3 differential manifest");
    assert_eq!(manifest.reference.product, "veraPDF");
    assert_eq!(manifest.reference.version, "1.30.2");
    assert_eq!(manifest.reference.source_manifest, SOURCE_MANIFEST);

    let source: SourceManifest = serde_json::from_slice(
        &fs::read(&manifest.reference.source_manifest).expect("read source differential manifest"),
    )
    .expect("parse source differential manifest");
    let source_paths = source
        .checked_in_mutations
        .into_iter()
        .map(|case| case.path)
        .collect::<BTreeSet<_>>();
    assert_eq!(source_paths.len(), manifest.mutation_count);

    let profiles = manifest
        .profiles
        .iter()
        .map(|profile| match profile.as_str() {
            "2a" => ReferenceProfile::PdfA2a,
            "2b" => ReferenceProfile::PdfA2b,
            "2u" => ReferenceProfile::PdfA2u,
            "3a" => ReferenceProfile::PdfA3a,
            "3b" => ReferenceProfile::PdfA3b,
            "3u" => ReferenceProfile::PdfA3u,
            other => panic!("unknown PDF/A-2/3 profile {other}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(profiles.len(), 6);

    let temporary =
        env::temp_dir().join(format!("page-pdfa-2-3-differential-{}", std::process::id()));
    fs::create_dir_all(&temporary).expect("create PDF/A-2/3 differential directory");

    for profile in profiles {
        let mut config = ReferenceConfig::pinned(executable.clone());
        config.profile = profile;
        config.coverage_gap_policy = CoverageGapPolicy::RejectForCompleteProfile;
        let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
        let paths = source_paths
            .iter()
            .map(|source_path| {
                let file_name = source_path
                    .file_name()
                    .expect("mutation path has a file name")
                    .to_string_lossy();
                let path = temporary.join(format!("{}-{file_name}", profile));
                let bytes = reidentify_profile(
                    &fs::read(source_path)
                        .unwrap_or_else(|error| panic!("read {}: {error}", source_path.display())),
                    profile,
                );
                fs::write(&path, bytes).expect("write profile-specific mutation");
                path
            })
            .collect::<Vec<_>>();

        for (source_path, report) in source_paths
            .iter()
            .zip(runner.compare_files(&paths, &SafetyLimits::default()))
        {
            assert!(
                matches!(
                    report.classification,
                    ComparisonClassification::Agreement
                        | ComparisonClassification::BothNoncompliant
                ),
                "{} under {profile}: {report}",
                source_path.display()
            );
        }
    }

    fs::remove_dir_all(temporary).expect("remove PDF/A-2/3 differential directory");
}

#[test]
fn pdfa_2_and_3_atomic_rule_evidence_is_complete_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        eprintln!("VERAPDF_BIN is unset; skipping PDF/A-2/3 atomic evidence");
        return;
    };

    let mapping = serde_json::from_slice::<Value>(
        &fs::read("tests/fixtures/pdfa-2-3-coverage.json")
            .expect("read PDF/A-2/3 coverage inventory"),
    )
    .expect("parse PDF/A-2/3 coverage inventory");
    let mappings = mapping["rule_mapping"]["mappings"]
        .as_array()
        .expect("PDF/A-2/3 mappings");
    let source_manifest: Value = serde_json::from_slice(
        &fs::read(SOURCE_MANIFEST).expect("read source differential manifest"),
    )
    .expect("parse source differential manifest");
    let mut candidates_by_rule = atomic_candidates(&source_manifest)
        .iter()
        .flat_map(|candidate| {
            candidate
                .local_rule_ids
                .iter()
                .map(move |rule| (rule.clone(), candidate.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    for mutation in source_manifest["checked_in_mutations"]
        .as_array()
        .expect("checked-in mutations")
    {
        let local_rule_id = mutation["local_rule_id"]
            .as_str()
            .expect("checked-in mutation local rule")
            .to_owned();
        let path = PathBuf::from(mutation["path"].as_str().expect("checked-in mutation path"));
        candidates_by_rule.insert(
            local_rule_id.clone(),
            AtomicCandidate {
                family: "checked_in".to_owned(),
                case: path.to_string_lossy().into_owned(),
                path: Some(path),
                reidentify: !local_rule_id.contains("-ID-"),
                local_rule_ids: vec![local_rule_id],
                kind: EvidenceKind::Fail,
            },
        );
    }
    for candidate in explicit_candidates() {
        for local_rule_id in &candidate.local_rule_ids {
            candidates_by_rule.insert(local_rule_id.clone(), candidate.clone());
        }
    }
    let inapplicable_baseline = AtomicCandidate {
        family: "document_feature".to_owned(),
        case: "baseline".to_owned(),
        path: None,
        reidentify: true,
        local_rule_ids: Vec::new(),
        kind: EvidenceKind::Pass,
    };
    for canonical in inapplicable_rules() {
        candidates_by_rule.insert(
            (*canonical).to_owned(),
            AtomicCandidate {
                kind: EvidenceKind::Inapplicable,
                ..inapplicable_baseline.clone()
            },
        );
    }
    let missing_candidates = mappings
        .iter()
        .filter_map(|mapping| {
            let canonical = mapping["canonical_local_rule_id"].as_str()?;
            (!candidates_by_rule.contains_key(canonical)).then_some(canonical)
        })
        .collect::<BTreeSet<_>>();
    assert!(
        missing_candidates.is_empty(),
        "every mapped PDF/A-2/3 rule needs a dedicated atomic candidate; missing: {missing_candidates:?}"
    );
    let profiles = [
        (ReferenceProfile::PdfA2a, "2a"),
        (ReferenceProfile::PdfA2b, "2b"),
        (ReferenceProfile::PdfA2u, "2u"),
        (ReferenceProfile::PdfA3a, "3a"),
        (ReferenceProfile::PdfA3b, "3b"),
        (ReferenceProfile::PdfA3u, "3u"),
    ];
    let temporary = env::temp_dir().join(format!(
        "page-pdfa-2-3-atomic-evidence-{}",
        std::process::id()
    ));
    fs::create_dir_all(&temporary).expect("create PDF/A-2/3 atomic evidence directory");

    let mut missing = Vec::new();
    for (profile, profile_name) in profiles {
        let mut config = ReferenceConfig::pinned(executable.clone());
        config.profile = profile;
        config.coverage_gap_policy = CoverageGapPolicy::RejectForCompleteProfile;
        let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
        let applicable = mappings
            .iter()
            .filter(|mapping| {
                mapping["applicable_profiles"]
                    .as_array()
                    .expect("applicable profiles")
                    .iter()
                    .any(|value| value == profile_name)
            })
            .collect::<Vec<_>>();
        let selected = applicable
            .iter()
            .filter_map(|mapping| {
                let canonical = mapping["canonical_local_rule_id"].as_str()?;
                candidates_by_rule.get(canonical).cloned()
            })
            .collect::<BTreeSet<_>>();
        let mut selected = selected.into_iter().collect::<Vec<_>>();
        selected
            .sort_by(|left, right| (&left.family, &left.case).cmp(&(&right.family, &right.case)));
        let paths = selected
            .iter()
            .enumerate()
            .map(|(index, candidate)| {
                let path = temporary.join(format!(
                    "{profile_name}-atomic-{index}-{}.pdf",
                    candidate.case.replace(['/', '\\'], "_")
                ));
                let source = candidate_fixture(candidate);
                let bytes = if candidate.reidentify {
                    reidentify_profile(&source, profile)
                } else {
                    source
                };
                fs::write(&path, bytes).expect("write atomic evidence fixture");
                path
            })
            .collect::<Vec<_>>();
        let reports = runner.compare_files(&paths, &SafetyLimits::default());
        let reports_by_case = selected
            .iter()
            .zip(reports)
            .map(|(candidate, report)| ((candidate.family.clone(), candidate.case.clone()), report))
            .collect::<BTreeMap<_, _>>();

        for mapping in applicable {
            let canonical = mapping["canonical_local_rule_id"]
                .as_str()
                .expect("canonical local rule");
            let candidate = candidates_by_rule
                .get(canonical)
                .expect("baseline evidence candidate");
            let local_rule = profile_local_rule_id(profile, canonical);
            let reference_rule = mapping["verapdf_rule_id"]
                .as_str()
                .expect("veraPDF rule id");
            let report = reports_by_case
                .get(&(candidate.family.clone(), candidate.case.clone()))
                .expect("selected atomic evidence report");
            let local_failed = report
                .local_report
                .failures
                .iter()
                .any(|failure| failure.rule_id == local_rule);
            let reference_failed = report.reference_result.as_ref().is_some_and(|result| {
                result
                    .failed_rule_ids
                    .iter()
                    .any(|id| id.to_string() == reference_rule)
            });
            let satisfied = match candidate.kind {
                EvidenceKind::Fail => local_failed && reference_failed,
                EvidenceKind::Pass | EvidenceKind::Inapplicable => {
                    !local_failed && !reference_failed
                }
                EvidenceKind::ReferenceParserDiscrepancy => {
                    !reference_failed
                        && report.classification
                            == ComparisonClassification::ReferenceParserDiscrepancy
                }
                EvidenceKind::Unrepresentable => {
                    !local_failed
                        && !reference_failed
                        && matches!(
                            report.classification,
                            ComparisonClassification::Agreement
                                | ComparisonClassification::BothNoncompliant
                        )
                }
            };
            if !satisfied {
                missing.push(format!(
                    "{profile_name}:{canonical}:{} {:?} local_failed={local_failed} reference_failed={reference_failed} classification={:?} reference_rules={:?}",
                    candidate.case,
                    candidate.kind,
                    report.classification,
                    report.reference_result.as_ref().map(|result| &result.failed_rule_ids)
                ));
            }
        }
    }
    fs::remove_dir_all(&temporary).expect("remove PDF/A-2/3 atomic evidence directory");
    assert!(
        missing.is_empty(),
        "missing atomic rule evidence:\n{}",
        missing.join("\n")
    );
}

#[test]
fn pdfa_2_and_3_release_gate_requires_completed_inventory() {
    if env::var_os("PAGE_REQUIRE_PDFA23_COMPLETE").is_none() {
        return;
    }
    let inventory: Value = serde_json::from_slice(
        &fs::read("tests/fixtures/pdfa-2-3-coverage.json")
            .expect("read PDF/A-2/3 coverage inventory"),
    )
    .expect("parse PDF/A-2/3 coverage inventory");
    assert_eq!(
        inventory["completion_gate"]["status"], "complete",
        "PDF/A-2 and PDF/A-3 release gate requested, but the inventory is still developing"
    );
}

fn atomic_candidates(manifest: &Value) -> Vec<AtomicCandidate> {
    let object = manifest.as_object().expect("differential manifest object");
    object
        .iter()
        .filter(|(key, value)| key.starts_with("atomic_") && value.is_array())
        .flat_map(|(key, value)| {
            let family = key
                .strip_prefix("atomic_")
                .and_then(|key| key.strip_suffix("_cases"))
                .expect("atomic family")
                .to_owned();
            value
                .as_array()
                .expect("atomic case array")
                .iter()
                .filter_map({
                    let family = family.clone();
                    move |case| {
                        let local_rule_ids = case["expected_local_failed_rule_ids"]
                            .as_array()?
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect::<Vec<_>>();
                        let name = case["name"].as_str()?.to_owned();
                        (!local_rule_ids.is_empty()).then_some(AtomicCandidate {
                            family: family.clone(),
                            case: name,
                            path: None,
                            reidentify: !local_rule_ids.iter().any(|id| id.contains("-ID-")),
                            local_rule_ids,
                            kind: EvidenceKind::Fail,
                        })
                    }
                })
        })
        .collect()
}

fn inapplicable_rules() -> &'static [&'static str] {
    &["PDFA1B-CMAP-REFERENCE-001"]
}

fn explicit_candidates() -> Vec<AtomicCandidate> {
    let case = |family: &str, name: &str, local_rule_id: &str| AtomicCandidate {
        family: family.to_owned(),
        case: name.to_owned(),
        path: None,
        reidentify: !local_rule_id.contains("-ID-"),
        local_rule_ids: vec![local_rule_id.to_owned()],
        kind: EvidenceKind::Fail,
    };
    let inapplicable = |family: &str, name: &str, local_rule_id: &str| AtomicCandidate {
        kind: EvidenceKind::Inapplicable,
        ..case(family, name, local_rule_id)
    };
    let checked_in = |path: &str, local_rule_id: &str| AtomicCandidate {
        family: "checked_in".to_owned(),
        case: path.to_owned(),
        path: Some(PathBuf::from(path)),
        reidentify: !local_rule_id.contains("-ID-"),
        local_rule_ids: vec![local_rule_id.to_owned()],
        kind: EvidenceKind::Fail,
    };
    let reference_parser_boundary = |path: &str, local_rule_id: &str| AtomicCandidate {
        family: "checked_in".to_owned(),
        case: path.to_owned(),
        path: Some(PathBuf::from(path)),
        reidentify: false,
        local_rule_ids: vec![local_rule_id.to_owned()],
        kind: EvidenceKind::ReferenceParserDiscrepancy,
    };
    let unrepresentable = |path: &str, local_rule_id: &str| AtomicCandidate {
        kind: EvidenceKind::Unrepresentable,
        ..checked_in(path, local_rule_id)
    };

    vec![
        checked_in(
            "tests/fixtures/mutations/PDFA1A-ID-CONFORMANCE-001/id_conformance_b.pdf",
            "PDFA1A-ID-CONFORMANCE-001",
        ),
        case(
            "document_feature",
            "tagged_missing",
            "PDFA1A-TAGGED-DOCUMENT-001",
        ),
        case(
            "document_feature",
            "struct_tree_missing",
            "PDFA1A-STRUCT-TREE-ROOT-001",
        ),
        case(
            "document_feature",
            "struct_tree_role_map_wrong_type",
            "PDFA1A-STRUCT-TREE-ROLE-MAP-001",
        ),
        case(
            "document_feature",
            "struct_tree_role_map_self_cycle",
            "PDFA1A-STRUCT-TREE-ROLE-MAP-CYCLE-001",
        ),
        case(
            "document_feature",
            "lang_catalog_invalid",
            "PDFA1A-LANG-001",
        ),
        checked_in(
            "tests/fixtures/mutations/PDFA1A-UNICODE-MAPPING-001/unicode_missing.pdf",
            "PDFA1A-UNICODE-MAPPING-001",
        ),
        case(
            "font",
            "unicode_pua_missing_actual_text",
            "PDFA1A-UNICODE-PUA-ACTUALTEXT-001",
        ),
        case(
            "document_feature",
            "struct_tree_role_map_standard_remap",
            "PDFA1A-STRUCT-TREE-ROLE-MAP-STANDARD-001",
        ),
        case(
            "document_feature",
            "permissions_invalid",
            "PDFA1B-PERMS-ENTRIES-001",
        ),
        case(
            "document_feature",
            "signature_reference_digest",
            "PDFA1B-SIGNATURE-REFERENCE-001",
        ),
        case(
            "metadata",
            "unknown_rdf_property",
            "PDFA1B-XMP-PROPERTY-DEFINITION-001",
        ),
        case(
            "graphics",
            "inline_image_lzw",
            "PDFA1B-INLINE-IMAGE-FILTER-001",
        ),
        case("font", "unicode_reserved", "PDFA1B-UNICODE-VALUE-001"),
        case("font", "type3_notdef", "PDFA1B-NOTDEF-GLYPH-001"),
        case(
            "document_feature",
            "embedded_file_invalid_pdfa",
            "PDFA1B-EMBEDDED-FILE-PDFA-001",
        ),
        case(
            "document_feature",
            "file_spec_with_ef",
            "PDFA1B-FILE-SPEC-F-AND-UF-001",
        ),
        case(
            "document_feature",
            "file_spec_with_ef",
            "PDFA1B-EMBEDDED-FILE-MIME-001",
        ),
        case(
            "document_feature",
            "file_spec_missing_f_uf",
            "PDFA1B-FILE-SPEC-F-AND-UF-001",
        ),
        case(
            "document_feature",
            "embedded_file_missing_mime",
            "PDFA1B-EMBEDDED-FILE-MIME-001",
        ),
        case(
            "annotation",
            "flags_missing",
            "PDFA1B-ANNOTATION-FLAGS-PRESENT-001",
        ),
        case(
            "annotation",
            "appearance_absent",
            "PDFA1B-ANNOTATION-AP-REQUIRED-001",
        ),
        case(
            "document_feature",
            "alternate_presentations",
            "PDFA1B-ALTERNATE-PRESENTATIONS-001",
        ),
        case(
            "action",
            "page_additional_action",
            "PDFA1B-PAGE-ADDITIONAL-ACTIONS-001",
        ),
        case("form", "xfa_present", "PDFA1B-ACROFORM-XFA-001"),
        case(
            "document_feature",
            "catalog_requirements",
            "PDFA1B-CATALOG-REQUIREMENTS-001",
        ),
        case(
            "font",
            "composite_unknown_named_cmap",
            "PDFA1B-CMAP-EMBEDDING-001",
        ),
        case(
            "color_path",
            "device_output_invalid_rgb",
            "PDFA1B-OUTPUTINTENT-PROFILE-REF-001",
        ),
        case(
            "output_intent",
            "pdfx_with_dest_output_profile_ref",
            "PDFA1B-OUTPUTINTENT-PROFILE-REF-001",
        ),
        case(
            "graphics",
            "extgstate_htp_present",
            "PDFA1B-EXTGSTATE-HTP-001",
        ),
        case(
            "graphics",
            "halftone_type_invalid",
            "PDFA1B-HALFTONE-TYPE-001",
        ),
        case(
            "graphics",
            "halftone_name_present",
            "PDFA1B-HALFTONE-NAME-001",
        ),
        inapplicable("font", "font_subtype_invalid", "PDFA1B-FONT-SUBTYPE-001"),
        case(
            "font",
            "font_file_subtype_invalid_fontfile3",
            "PDFA1B-FONT-FILE-SUBTYPE-001",
        ),
        case(
            "font",
            "tt_nonsymbolic_zero_cmaps",
            "PDFA1B-TRUETYPE-NONSYMBOLIC-CMAP-001",
        ),
        case(
            "object_limit",
            "object_real_pdfa2_high",
            "PDFA1B-REAL-RANGE-001",
        ),
        case(
            "object_limit",
            "object_real_pdfa2_minimum",
            "PDFA1B-REAL-MINIMUM-001",
        ),
        case(
            "document_feature",
            "page_boundary_too_small",
            "PDFA1B-PAGE-BOUNDARY-001",
        ),
        case(
            "device_color",
            "devicen_33_components",
            "PDFA1B-DEVICEN-COMPONENTS-001",
        ),
        case(
            "graphics",
            "extgstate_bm_invalid",
            "PDFA1B-EXTGSTATE-BLEND-MODE-001",
        ),
        case("xobject", "image_bpc_3", "PDFA1B-IMAGE-BPC-001"),
        case(
            "annotation",
            "subtype_unknown",
            "PDFA1B-ANNOTATION-SUBTYPE-001",
        ),
        case(
            "annotation",
            "flags_invisible",
            "PDFA1B-ANNOTATION-FLAGS-001",
        ),
        case(
            "document_feature",
            "ocproperties_missing_name",
            "PDFA1B-OPTIONAL-CONTENT-NAME-001",
        ),
        case(
            "document_feature",
            "ocproperties_duplicate_name",
            "PDFA1B-OPTIONAL-CONTENT-DUPLICATE-NAME-001",
        ),
        case(
            "document_feature",
            "ocproperties_order_missing",
            "PDFA1B-OPTIONAL-CONTENT-ORDER-001",
        ),
        case(
            "document_feature",
            "ocproperties_as_present",
            "PDFA1B-OPTIONAL-CONTENT-AS-001",
        ),
        case(
            "font",
            "unicode_name_basefont_invalid",
            "PDFA1B-UNICODE-NAME-001",
        ),
        case(
            "color_path",
            "separation_inconsistent",
            "PDFA1B-SEPARATION-CONSISTENCY-001",
        ),
        case(
            "icc_cmyk_overprint",
            "stroke_opm_one",
            "PDFA1B-ICCBased-CMYK-OVERPRINT-001",
        ),
        case(
            "graphics",
            "halftone_transfer_root_invalid",
            "PDFA1B-HALFTONE-TRANSFER-FUNCTION-001",
        ),
        case(
            "graphics",
            "inherited_resource_calgray",
            "PDFA1B-CONTENT-RESOURCES-001",
        ),
        case(
            "graphics",
            "extgstate_transparency_no_output_intent",
            "PDFA1B-TRANSPARENCY-GROUP-CS-001",
        ),
        case(
            "metadata",
            "xmp_utf16le_without_bom",
            "PDFA1B-XMP-ENCODING-001",
        ),
        case(
            "document_feature",
            "stream_lzwdecode",
            "PDFA1B-STREAM-FILTER-001",
        ),
        case("metadata", "wrong_part_four", "PDFA1B-ID-PART-001"),
        case("metadata", "id_corr_prefix", "PDFA1B-ID-CORR-PREFIX-001"),
        case(
            "pdfa_2_3",
            "catalog_needs_rendering",
            "PDFA1B-CATALOG-NEEDS-RENDERING-001",
        ),
        case(
            "pdfa_2_3",
            "devicen_colorants",
            "PDFA1B-DEVICEN-COLORANTS-001",
        ),
        case(
            "pdfa_2_3",
            "file_spec_af_relationship",
            "PDFA1B-FILE-SPEC-AF-RELATIONSHIP-001",
        ),
        case(
            "pdfa_2_3",
            "file_spec_association",
            "PDFA1B-FILE-SPEC-ASSOCIATION-001",
        ),
        case(
            "pdfa_2_3",
            "jpeg2000_bit_depth",
            "PDFA1B-JPEG2000-BIT-DEPTH-001",
        ),
        case(
            "pdfa_2_3",
            "jpeg2000_channels",
            "PDFA1B-JPEG2000-CHANNELS-001",
        ),
        case(
            "pdfa_2_3",
            "jpeg2000_color_method",
            "PDFA1B-JPEG2000-COLOR-METHOD-001",
        ),
        case(
            "pdfa_2_3",
            "jpeg2000_color_space",
            "PDFA1B-JPEG2000-COLOR-SPACE-001",
        ),
        case(
            "pdfa_2_3",
            "jpeg2000_color_specs",
            "PDFA1B-JPEG2000-COLOR-SPECS-001",
        ),
        case("pdfa_2_3", "pres_steps", "PDFA1B-PRES-STEPS-001"),
        case(
            "pdfa_2_3",
            "signature_byte_range",
            "PDFA1B-SIGNATURE-BYTERANGE-001",
        ),
        case(
            "pdfa_2_3",
            "signature_certificate",
            "PDFA1B-SIGNATURE-CERTIFICATE-001",
        ),
        case(
            "pdfa_2_3",
            "signature_signer_count",
            "PDFA1B-SIGNATURE-SIGNER-COUNT-001",
        ),
        reference_parser_boundary("tests/fixtures/encrypted.pdf", "PDFA1B-ENCRYPTION-001"),
        unrepresentable(
            "tests/fixtures/typst-pdfa-1b.pdf",
            "PDFA1B-INDIRECT-OBJECT-COUNT-001",
        ),
    ]
}

fn atomic_fixture(candidate: &AtomicCandidate) -> Vec<u8> {
    match candidate.family.as_str() {
        "metadata" => common::metadata_fixture(&candidate.case),
        "output_intent" => common::output_intent_fixture(&candidate.case),
        "icc_based" => common::icc_based_fixture(&candidate.case),
        "device_color" => common::device_color_fixture(&candidate.case),
        "color_path" => common::color_path_fixture(&candidate.case),
        "xobject" => common::xobject_fixture(&candidate.case),
        "graphics" => common::graphics_fixture(&candidate.case),
        "font" | "composite_font" | "truetype" => common::font_fixture(&candidate.case),
        "icc_cmyk_overprint" => common::icc_cmyk_overprint_fixture(&candidate.case),
        "font_content_source" => common::font_content_source_fixture(&candidate.case),
        "type0_descendant" => common::type0_descendant_fixture(&candidate.case),
        "transparency" => common::graphics_fixture(&candidate.case),
        "annotation" => common::annotation_fixture(&candidate.case),
        "action" => common::action_fixture(&candidate.case),
        "form" => common::form_fixture(&candidate.case),
        "document_feature" => common::document_feature_fixture(&candidate.case),
        "pdfa_2_3" => common::pdfa_2_3_fixture(&candidate.case),
        "object_limit" => common::object_limit_fixture(&candidate.case),
        "syntax" => common::syntax_fixture(&candidate.case),
        family => panic!("unknown atomic family {family}"),
    }
}

fn candidate_fixture(candidate: &AtomicCandidate) -> Vec<u8> {
    candidate.path.as_ref().map_or_else(
        || atomic_fixture(candidate),
        |path| fs::read(path).expect("read mutation"),
    )
}

fn profile_local_rule_id(profile: ReferenceProfile, canonical: &str) -> String {
    let prefix = match profile {
        ReferenceProfile::PdfA2a | ReferenceProfile::PdfA3a => {
            if matches!(profile, ReferenceProfile::PdfA2a) {
                "PDFA2A"
            } else {
                "PDFA3A"
            }
        }
        ReferenceProfile::PdfA2b => "PDFA2B",
        ReferenceProfile::PdfA2u => "PDFA2U",
        ReferenceProfile::PdfA3b => "PDFA3B",
        ReferenceProfile::PdfA3u => "PDFA3U",
        ReferenceProfile::PdfA1a | ReferenceProfile::PdfA1b => unreachable!(),
    };
    canonical
        .strip_prefix("PDFA1A-")
        .or_else(|| canonical.strip_prefix("PDFA1B-"))
        .map_or_else(
            || canonical.to_owned(),
            |suffix| format!("{prefix}-{suffix}"),
        )
}

fn reidentify_profile(bytes: &[u8], profile: ReferenceProfile) -> Vec<u8> {
    let mut bytes = bytes.to_vec();
    let part = match profile {
        ReferenceProfile::PdfA2a | ReferenceProfile::PdfA2b | ReferenceProfile::PdfA2u => b'2',
        ReferenceProfile::PdfA3a | ReferenceProfile::PdfA3b | ReferenceProfile::PdfA3u => b'3',
        ReferenceProfile::PdfA1a | ReferenceProfile::PdfA1b => unreachable!(),
    };
    let conformance = match profile {
        ReferenceProfile::PdfA2a | ReferenceProfile::PdfA3a => b'A',
        ReferenceProfile::PdfA2b | ReferenceProfile::PdfA3b => b'B',
        ReferenceProfile::PdfA2u | ReferenceProfile::PdfA3u => b'U',
        ReferenceProfile::PdfA1a | ReferenceProfile::PdfA1b => unreachable!(),
    };
    replace_first(
        &mut bytes,
        b"<pdfaid:part>1</pdfaid:part>",
        b"<pdfaid:part>2</pdfaid:part>",
        part,
    );
    replace_first(&mut bytes, b"pdfaid:part=\"1\"", b"pdfaid:part=\"2\"", part);
    replace_first(
        &mut bytes,
        b"<pdfaid:conformance>A</pdfaid:conformance>",
        b"<pdfaid:conformance>B</pdfaid:conformance>",
        conformance,
    );
    replace_first(
        &mut bytes,
        b"<pdfaid:conformance>B</pdfaid:conformance>",
        b"<pdfaid:conformance>A</pdfaid:conformance>",
        conformance,
    );
    replace_first(
        &mut bytes,
        b"pdfaid:conformance=\"A\"",
        b"pdfaid:conformance=\"B\"",
        conformance,
    );
    replace_first(
        &mut bytes,
        b"pdfaid:conformance=\"B\"",
        b"pdfaid:conformance=\"A\"",
        conformance,
    );
    bytes
}

fn replace_first(bytes: &mut [u8], old: &[u8], new: &[u8], value: u8) {
    let Some(start) = bytes.windows(old.len()).position(|window| window == old) else {
        return;
    };
    let offset = old
        .iter()
        .position(|byte| *byte == b'1' || *byte == b'A' || *byte == b'B')
        .expect("profile marker contains a replaceable value");
    assert_eq!(old.len(), new.len());
    bytes[start + offset] = value;
}
