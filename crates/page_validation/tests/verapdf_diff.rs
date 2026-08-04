use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use page_validation::SafetyLimits;
use page_validation::differential::{
    ComparisonClassification, DifferentialRunner, PINNED_VERAPDF_PROFILE, PINNED_VERAPDF_VERSION,
    ReferenceConfig, ReferenceProfile,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub mod common;

#[test]
#[ignore = "maintenance generator for the checked canonical form fixture"]
fn regenerate_canonical_form_fixture() {
    fs::write(
        "tests/fixtures/canonical-pdfa-1a-forms.pdf",
        common::canonical_form_fixture(),
    )
    .expect("write canonical form fixture");
}

#[test]
#[ignore = "maintenance generator for the checked unsupported-font applicability fixture"]
fn regenerate_canonical_unused_invalid_font_fixture() {
    fs::write(
        "tests/fixtures/canonical-pdfa-1a-unused-invalid-font.pdf",
        common::canonical_a1a_unused_invalid_font_fixture(),
    )
    .expect("write unused invalid-font fixture");
}

#[test]
#[ignore = "maintenance generator for checked canonical A-1a mutations"]
fn regenerate_canonical_a1a_mutations() {
    let cases = [
        ("PDFA1A-ID-CONFORMANCE-001", "id_conformance_b"),
        ("PDFA1A-TAGGED-DOCUMENT-001", "tagged_missing"),
        ("PDFA1A-STRUCT-TREE-ROOT-001", "struct_tree_missing"),
        ("PDFA1A-STRUCT-TREE-ROLE-MAP-001", "role_map_wrong_type"),
        ("PDFA1A-STRUCT-TREE-ROLE-MAP-CYCLE-001", "role_map_cycle"),
        ("PDFA1A-LANG-001", "language_missing"),
        ("PDFA1A-UNICODE-MAPPING-001", "unicode_missing"),
    ];
    for (rule_id, case) in cases {
        let directory = format!("tests/fixtures/mutations/{rule_id}");
        fs::create_dir_all(&directory).expect("create canonical mutation directory");
        fs::write(
            format!("{directory}/{case}.pdf"),
            common::canonical_a1a_mutation(case),
        )
        .expect("write canonical A-1a mutation");
    }
}

#[test]
#[ignore = "maintenance generator for checked shared PDF/A-1 mutation fixtures"]
fn regenerate_shared_mutation_fixtures() {
    let manifest_path = Path::new("tests/fixtures/verapdf-diff-cases.json");
    let mut manifest: Value = serde_json::from_slice(
        &fs::read(manifest_path).expect("read shared differential manifest"),
    )
    .expect("parse shared differential manifest");
    let coverage: Value = serde_json::from_slice(
        &fs::read("tests/fixtures/pdfa-1b-coverage.json").expect("read coverage inventory"),
    )
    .expect("parse coverage inventory");
    let mut mapped = BTreeMap::<String, String>::new();
    for predicate in coverage["predicates"]
        .as_array()
        .expect("coverage predicates")
    {
        let reference = predicate["verapdf_rule_id"]
            .as_str()
            .expect("coverage reference rule id");
        for local in predicate["local_checks"]
            .as_array()
            .expect("coverage local checks")
        {
            mapped.insert(
                local.as_str().expect("coverage local check").to_owned(),
                reference.to_owned(),
            );
        }
    }

    let mut selected = BTreeSet::new();
    let mut checked_in = Vec::new();
    let object = manifest.as_object().expect("manifest object");
    let atomic_keys = object
        .keys()
        .filter(|key| key.starts_with("atomic_") && key.ends_with("_cases"))
        .cloned()
        .collect::<Vec<_>>();
    for key in atomic_keys {
        let family = key.trim_start_matches("atomic_").trim_end_matches("_cases");
        let baseline = atomic_baseline_case(family);
        for case in manifest[&key].as_array().expect("atomic cases") {
            let local_failed = case["expected_local_failed_rule_ids"]
                .as_array()
                .expect("atomic local failures");
            let reference_failed = case["expected_verapdf_failed_rule_ids"]
                .as_array()
                .expect("atomic reference failures");
            for local in local_failed {
                let Some(local_rule) = local.as_str() else {
                    continue;
                };
                let Some(reference_rule) = mapped.get(local_rule) else {
                    continue;
                };
                if !reference_failed
                    .iter()
                    .any(|id| id.as_str() == Some(reference_rule))
                    || !selected.insert(local_rule.to_owned())
                {
                    continue;
                }
                let case_name = case["name"].as_str().expect("atomic case name");
                let relative = format!(
                    "tests/fixtures/mutations/{local_rule}/shared-{family}-{case_name}.pdf"
                );
                let bytes = shared_mutation_fixture(family, case_name);
                if let Some(parent) = Path::new(&relative).parent() {
                    fs::create_dir_all(parent).expect("create shared mutation directory");
                }
                fs::write(&relative, &bytes).expect("write shared mutation fixture");
                checked_in.push(json!({
                    "path": relative,
                    "family": family,
                    "case": case_name,
                    "baseline_case": baseline,
                    "local_rule_id": local_rule,
                    "reference_rule_id": reference_rule,
                    "expected_local_failed_rule_ids": case["expected_local_failed_rule_ids"],
                    "expected_local_passed_rule_ids": case["expected_local_passed_rule_ids"],
                    "expected_verapdf_failed_rule_ids": case["expected_verapdf_failed_rule_ids"],
                    "expected_verapdf_passed_rule_ids": case["expected_verapdf_passed_rule_ids"],
                    "sha256": hex_sha256(&bytes),
                    "rationale": case["rationale"],
                }));
            }
        }
    }
    let blend_case = "extgstate_bm_multiply";
    let blend_path = "tests/fixtures/mutations/PDFA1B-EXTGSTATE-BLEND-MODE-001/shared-graphics-extgstate_bm_multiply.pdf";
    let blend_bytes = common::graphics_fixture(blend_case);
    fs::create_dir_all(
        Path::new(blend_path)
            .parent()
            .expect("blend mutation parent"),
    )
    .expect("create blend mutation directory");
    fs::write(blend_path, &blend_bytes).expect("write blend mutation fixture");
    checked_in.push(json!({
        "path": blend_path,
        "family": "graphics",
        "case": blend_case,
        "baseline_case": "baseline",
        "local_rule_id": "PDFA1B-EXTGSTATE-BLEND-MODE-001",
        "reference_rule_id": "ISO 19005-1:2005:6.4:4",
        "expected_local_failed_rule_ids": ["PDFA1B-EXTGSTATE-BLEND-MODE-001"],
        "expected_local_passed_rule_ids": [],
        "expected_verapdf_failed_rule_ids": ["ISO 19005-1:2005:6.4:4"],
        "expected_verapdf_passed_rule_ids": [],
        "sha256": hex_sha256(&blend_bytes),
        "rationale": "The checked-in graphics mutation changes one ExtGState blend mode from the compliant graphics baseline."
    }));
    for (source, local_rule, reference_rule, case_name, rationale) in [
        (
            "header-binary-comment-invalid.pdf",
            "PDFA1B-HEADER-BINARY-COMMENT-001",
            "ISO 19005-1:2005:6.1.2:2",
            "header_binary_comment_invalid",
            "The checked-in parser fixture changes only the required binary comment after the PDF header.",
        ),
        (
            "indirect-object-syntax.pdf",
            "PDFA1B-INDIRECT-OBJECT-SYNTAX-001",
            "ISO 19005-1:2005:6.1.8:1",
            "indirect_object_syntax",
            "The checked-in parser fixture changes only the indirect-object syntax.",
        ),
        (
            "stream-eol-invalid.pdf",
            "PDFA1B-STREAM-EOL-001",
            "ISO 19005-1:2005:6.1.7:2",
            "stream_eol_invalid",
            "The checked-in parser fixture changes only the stream end-of-line marker.",
        ),
        (
            "xref-eol.pdf",
            "PDFA1B-XREF-EOL-001",
            "ISO 19005-1:2005:6.1.4:2",
            "xref_eol",
            "The checked-in parser fixture changes only the cross-reference end-of-line marker.",
        ),
        (
            "xref-stream.pdf",
            "PDFA1B-XREF-STREAM-001",
            "ISO 19005-1:2005:6.1.4:3",
            "xref_stream",
            "The checked-in parser fixture changes only the forbidden cross-reference stream form.",
        ),
        (
            "xref-spacing.pdf",
            "PDFA1B-XREF-SUBSECTION-SPACING-001",
            "ISO 19005-1:2005:6.1.4:1",
            "xref_spacing",
            "The checked-in parser fixture changes only the spacing in the cross-reference subsection header.",
        ),
    ] {
        let source_path = format!("tests/fixtures/{source}");
        let bytes = fs::read(&source_path).expect("read parser mutation source");
        let relative =
            format!("tests/fixtures/mutations/{local_rule}/shared-corpus-{case_name}.pdf");
        fs::create_dir_all(
            Path::new(&relative)
                .parent()
                .expect("parser mutation parent"),
        )
        .expect("create parser mutation directory");
        fs::write(&relative, &bytes).expect("write parser mutation fixture");
        checked_in.push(json!({
            "path": relative,
            "family": "corpus",
            "case": case_name,
            "baseline_case": "typst-pdfa-1b.pdf",
            "local_rule_id": local_rule,
            "reference_rule_id": reference_rule,
            "expected_local_failed_rule_ids": [local_rule],
            "expected_local_passed_rule_ids": [],
            "expected_verapdf_failed_rule_ids": [reference_rule],
            "expected_verapdf_passed_rule_ids": [],
            "sha256": hex_sha256(&bytes),
            "rationale": rationale,
        }));
    }
    let mut post_eof_bytes = fs::read("tests/fixtures/canonical-pdfa-1b.pdf")
        .expect("read canonical PDF/A-1b mutation base");
    post_eof_bytes.extend_from_slice(b"\npost-eof mutation");
    let post_eof_path =
        "tests/fixtures/mutations/PDFA1B-POST-EOF-DATA-001/shared-corpus-post_eof_canonical.pdf";
    fs::create_dir_all(
        Path::new(post_eof_path)
            .parent()
            .expect("post-EOF mutation parent"),
    )
    .expect("create post-EOF mutation directory");
    fs::write(post_eof_path, &post_eof_bytes).expect("write post-EOF mutation fixture");
    checked_in.push(json!({
        "path": post_eof_path,
        "family": "corpus",
        "case": "post_eof_canonical",
        "baseline_case": "typst-pdfa-1b.pdf",
        "local_rule_id": "PDFA1B-POST-EOF-DATA-001",
        "reference_rule_id": "ISO 19005-1:2005:6.1.3:3",
        "expected_local_failed_rule_ids": ["PDFA1B-POST-EOF-DATA-001"],
        "expected_local_passed_rule_ids": [],
        "expected_verapdf_failed_rule_ids": ["ISO 19005-1:2005:6.1.3:3"],
        "expected_verapdf_passed_rule_ids": [],
        "sha256": hex_sha256(&post_eof_bytes),
        "rationale": "The mutation appends bytes after the final EOF marker of the compliant canonical PDF/A-1b fixture."
    }));
    assert!(selected.iter().all(|local| mapped.contains_key(local)));
    manifest["checked_in_mutations"] = Value::Array(checked_in);
    fs::write(
        manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("serialize shared differential manifest"),
    )
    .expect("write shared differential manifest");
}

fn atomic_baseline_case(family: &str) -> &'static str {
    match family {
        "metadata" => "baseline_b",
        "output_intent"
        | "icc_based"
        | "device_color"
        | "xobject"
        | "graphics"
        | "font_content_source"
        | "type0_descendant"
        | "truetype"
        | "transparency"
        | "annotation"
        | "action"
        | "form"
        | "document_feature"
        | "object_limit"
        | "syntax" => "baseline",
        "color_path" => "icc_baseline",
        "font" => "baseline_embedded",
        "composite_font" => "composite_baseline",
        family => panic!("unknown atomic family {family}"),
    }
}

fn shared_mutation_fixture(family: &str, case: &str) -> Vec<u8> {
    match family {
        "metadata" => common::metadata_fixture(case),
        "output_intent" => common::output_intent_fixture(case),
        "icc_based" => common::icc_based_fixture(case),
        "device_color" => common::device_color_fixture(case),
        "color_path" => common::color_path_fixture(case),
        "xobject" => common::xobject_fixture(case),
        "graphics" => common::graphics_fixture(case),
        "font" | "composite_font" | "truetype" => common::font_fixture(case),
        "font_content_source" => common::font_content_source_fixture(case),
        "type0_descendant" => common::type0_descendant_fixture(case),
        "transparency" => common::graphics_fixture(case),
        "annotation" => common::annotation_fixture(case),
        "action" => common::action_fixture(case),
        "form" => common::form_fixture(case),
        "document_feature" => common::document_feature_fixture(case),
        "object_limit" => common::object_limit_fixture(case),
        "syntax" => common::syntax_fixture(case),
        "corpus" if case == "post_eof_canonical" => {
            let mut bytes = fs::read("tests/fixtures/canonical-pdfa-1b.pdf")
                .expect("read canonical PDF/A-1b mutation base");
            bytes.extend_from_slice(b"\npost-eof mutation");
            bytes
        }
        "corpus" => fs::read(format!("tests/fixtures/{case}")).expect("read corpus fixture"),
        family => panic!("unknown atomic family {family}"),
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Debug, Deserialize)]
struct Manifest {
    reference: ManifestReference,
    cases: Vec<ManifestCase>,
    #[serde(default)]
    checked_in_mutations: Vec<CheckedInMutation>,
    #[serde(default)]
    atomic_metadata_cases: Vec<AtomicRuleCase>,
    #[serde(default)]
    atomic_output_intent_cases: Vec<AtomicRuleCase>,
    #[serde(default)]
    atomic_icc_based_cases: Vec<AtomicRuleCase>,
    #[serde(default)]
    atomic_device_color_cases: Vec<AtomicRuleCase>,
    #[serde(default)]
    atomic_color_path_cases: Vec<AtomicRuleCase>,
    #[serde(default)]
    atomic_xobject_cases: Vec<AtomicRuleCase>,
    #[serde(default)]
    atomic_graphics_cases: Vec<AtomicRuleCase>,
    #[serde(default)]
    atomic_font_cases: Vec<AtomicRuleCase>,
    #[serde(default)]
    atomic_font_content_source_cases: Vec<AtomicRuleCase>,
    #[serde(default)]
    atomic_type0_descendant_cases: Vec<AtomicRuleCase>,
    #[serde(default)]
    atomic_composite_font_cases: Vec<AtomicRuleCase>,
    #[serde(default)]
    atomic_truetype_cases: Vec<AtomicRuleCase>,
    #[serde(default)]
    atomic_transparency_cases: Vec<AtomicRuleCase>,
    #[serde(default)]
    atomic_annotation_cases: Vec<AtomicRuleCase>,
    #[serde(default)]
    atomic_action_cases: Vec<AtomicRuleCase>,
    #[serde(default)]
    atomic_form_cases: Vec<AtomicRuleCase>,
    #[serde(default)]
    atomic_document_feature_cases: Vec<AtomicRuleCase>,
    #[serde(default)]
    atomic_object_limit_cases: Vec<AtomicRuleCase>,
    #[serde(default)]
    atomic_syntax_cases: Vec<AtomicRuleCase>,
}

#[derive(Debug, Deserialize)]
struct CheckedInMutation {
    path: PathBuf,
    family: String,
    case: String,
    baseline_case: String,
    local_rule_id: String,
    reference_rule_id: String,
    expected_local_failed_rule_ids: Vec<String>,
    expected_local_passed_rule_ids: Vec<String>,
    expected_verapdf_failed_rule_ids: Vec<String>,
    expected_verapdf_passed_rule_ids: Vec<String>,
    rationale: String,
}

#[derive(Debug, Deserialize)]
struct ManifestReference {
    version: String,
    profile: String,
}

#[derive(Debug, Deserialize)]
struct ManifestCase {
    path: PathBuf,
    expected_classification: ComparisonClassification,
    rationale: String,
    #[serde(default)]
    reference_baseline: Option<PathBuf>,
    #[serde(default)]
    expected_verapdf_added_rule_ids: Vec<String>,
    #[serde(default)]
    expected_verapdf_passed_rule_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AtomicRuleCase {
    name: String,
    expected_local_failed_rule_ids: Vec<String>,
    expected_local_passed_rule_ids: Vec<String>,
    expected_verapdf_failed_rule_ids: Vec<String>,
    expected_verapdf_passed_rule_ids: Vec<String>,
    rationale: String,
}

#[derive(Debug, Deserialize)]
struct PdfA1aManifest {
    reference: ManifestReference,
    shared_cases: Vec<PathBuf>,
    #[serde(default)]
    composition_cases: Vec<PathBuf>,
    #[serde(default)]
    mutation_cases: Vec<PdfA1aMutationCase>,
    #[serde(default)]
    applicability_cases: Vec<PdfA1aApplicabilityCase>,
    a_only_rules: Vec<PdfA1aRule>,
    #[serde(default)]
    upstream_failure_cases: Vec<PdfA1aUpstreamFailureCase>,
    #[serde(default)]
    upstream_mismatch_cases: Vec<PdfA1aUpstreamMismatchCase>,
}

#[derive(Debug, Deserialize)]
struct PdfA1aMutationCase {
    path: PathBuf,
    baseline: PathBuf,
    local_rule_id: String,
    reference_rule_id: String,
    rationale: String,
}

#[derive(Debug, Deserialize)]
struct PdfA1aApplicabilityCase {
    name: String,
    path: PathBuf,
    local_rule_id: String,
    reference_rule_id: String,
    rationale: String,
}

#[derive(Debug, Deserialize)]
struct PdfA1aRule {
    local_rule_id: String,
    reference_rule_id: String,
    cases: Vec<PdfA1aMatrixCase>,
}

#[derive(Debug, Deserialize)]
struct PdfA1aMatrixCase {
    name: String,
    fixture_family: String,
    status: String,
    expected_local_failure: bool,
    expected_verapdf_failure: bool,
    rationale: String,
}

#[derive(Debug, Deserialize)]
struct PdfA1aUpstreamFailureCase {
    name: String,
    fixture_family: String,
    expected_local_rule_id: String,
    rationale: String,
}

#[derive(Debug, Deserialize)]
struct PdfA1aUpstreamMismatchCase {
    name: String,
    fixture_family: String,
    expected_local_failure: bool,
    expected_verapdf_failure: bool,
    expected_classification: ComparisonClassification,
    local_rule_id: String,
    reference_rule_id: String,
    rationale: String,
}

#[test]
fn pinned_verapdf_manifest_matches_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        eprintln!("VERAPDF_BIN is unset; skipping opt-in veraPDF differential run");
        return;
    };
    let manifest_path = Path::new("tests/fixtures/verapdf-diff-cases.json");
    let manifest: Manifest =
        serde_json::from_slice(&fs::read(manifest_path).expect("read differential case manifest"))
            .expect("parse differential case manifest");
    assert_eq!(manifest.reference.version, PINNED_VERAPDF_VERSION);
    assert_eq!(
        manifest.reference.profile,
        PINNED_VERAPDF_PROFILE.as_verapdf_flavour()
    );

    let runner =
        DifferentialRunner::new(ReferenceConfig::pinned(executable)).expect("pinned veraPDF");
    let case_limit = env::var("PAGE_DIFF_CASE_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok());
    let case_offset = env::var("PAGE_DIFF_CASE_OFFSET")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let cases = manifest
        .cases
        .into_iter()
        .skip(case_offset)
        .take(case_limit.unwrap_or(usize::MAX))
        .collect::<Vec<_>>();
    let baseline_paths = cases
        .iter()
        .filter_map(|case| case.reference_baseline.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let baseline_ids = baseline_paths
        .iter()
        .cloned()
        .zip(
            runner
                .compare_files(&baseline_paths, &SafetyLimits::default())
                .into_iter()
                .map(|report| {
                    report
                        .reference_result
                        .expect("baseline reference result")
                        .failed_rule_ids
                        .iter()
                        .filter(|id| !is_untracked_1302_rule(id))
                        .map(ToString::to_string)
                        .collect::<BTreeSet<_>>()
                }),
        )
        .collect::<BTreeMap<_, _>>();
    let case_paths = cases
        .iter()
        .map(|case| case.path.clone())
        .collect::<Vec<_>>();
    let reports = runner.compare_files(&case_paths, &SafetyLimits::default());
    for (case, report) in cases.into_iter().zip(reports) {
        assert_eq!(
            report.classification,
            case.expected_classification,
            "{}: {}\n{report}",
            case.path.display(),
            case.rationale
        );
        if let Some(baseline_path) = case.reference_baseline {
            let actual_ids = report
                .reference_result
                .as_ref()
                .expect("case reference result")
                .failed_rule_ids
                .iter()
                .filter(|id| !is_untracked_1302_rule(id))
                .map(ToString::to_string)
                .collect::<BTreeSet<_>>();
            assert_rule_id_delta(
                "veraPDF",
                &case.path.display().to_string(),
                &case.rationale,
                baseline_ids.get(&baseline_path).expect("baseline IDs"),
                &actual_ids,
                &case.expected_verapdf_added_rule_ids,
                &[],
            );
        }
        if let Some(reference) = report.reference_result.as_ref() {
            let actual_ids = reference
                .failed_rule_ids
                .iter()
                .map(ToString::to_string)
                .collect::<BTreeSet<_>>();
            for expected in &case.expected_verapdf_passed_rule_ids {
                assert!(
                    !actual_ids.contains(expected),
                    "{}: expected veraPDF rule {expected} to pass; failures: {actual_ids:?}",
                    case.path.display()
                );
            }
        }
    }

    let temporary = std::env::temp_dir().join(format!(
        "page-verapdf-metadata-atomic-{}",
        std::process::id()
    ));
    fs::create_dir_all(&temporary).expect("create atomic fixture directory");
    assert_atomic_cases(
        &runner,
        &temporary,
        "metadata",
        "baseline_b",
        &manifest.atomic_metadata_cases,
        common::metadata_fixture,
    );
    assert_atomic_cases(
        &runner,
        &temporary,
        "output-intent",
        "baseline",
        &manifest.atomic_output_intent_cases,
        common::output_intent_fixture,
    );
    assert_atomic_cases(
        &runner,
        &temporary,
        "icc-based",
        "baseline",
        &manifest.atomic_icc_based_cases,
        common::icc_based_fixture,
    );
    assert_atomic_cases(
        &runner,
        &temporary,
        "device-color",
        "baseline",
        &manifest.atomic_device_color_cases,
        common::device_color_fixture,
    );
    assert_atomic_cases(
        &runner,
        &temporary,
        "color-path",
        "icc_baseline",
        &manifest.atomic_color_path_cases,
        common::color_path_fixture,
    );
    assert_atomic_cases(
        &runner,
        &temporary,
        "xobject",
        "baseline",
        &manifest.atomic_xobject_cases,
        common::xobject_fixture,
    );
    assert_atomic_cases(
        &runner,
        &temporary,
        "graphics",
        "baseline",
        &manifest.atomic_graphics_cases,
        common::graphics_fixture,
    );
    assert_atomic_cases(
        &runner,
        &temporary,
        "font",
        "baseline_embedded",
        &manifest.atomic_font_cases,
        common::font_fixture,
    );
    assert_atomic_cases(
        &runner,
        &temporary,
        "font-content-source",
        "baseline",
        &manifest.atomic_font_content_source_cases,
        common::font_content_source_fixture,
    );
    assert_atomic_cases(
        &runner,
        &temporary,
        "type0-descendant",
        "baseline",
        &manifest.atomic_type0_descendant_cases,
        common::type0_descendant_fixture,
    );
    assert_atomic_cases(
        &runner,
        &temporary,
        "composite-font",
        "composite_baseline",
        &manifest.atomic_composite_font_cases,
        common::font_fixture,
    );
    assert_atomic_cases(
        &runner,
        &temporary,
        "truetype",
        "baseline_embedded",
        &manifest.atomic_truetype_cases,
        common::font_fixture,
    );
    assert_atomic_cases(
        &runner,
        &temporary,
        "transparency",
        "baseline",
        &manifest.atomic_transparency_cases,
        common::graphics_fixture,
    );
    assert_atomic_cases(
        &runner,
        &temporary,
        "annotation",
        "baseline",
        &manifest.atomic_annotation_cases,
        common::annotation_fixture,
    );
    assert_atomic_cases(
        &runner,
        &temporary,
        "action",
        "baseline",
        &manifest.atomic_action_cases,
        common::action_fixture,
    );
    assert_atomic_cases(
        &runner,
        &temporary,
        "form",
        "baseline",
        &manifest.atomic_form_cases,
        common::form_fixture,
    );
    assert_atomic_cases(
        &runner,
        &temporary,
        "document-feature",
        "baseline",
        &manifest.atomic_document_feature_cases,
        common::document_feature_fixture,
    );
    assert_atomic_cases(
        &runner,
        &temporary,
        "object-limit",
        "baseline",
        &manifest.atomic_object_limit_cases,
        common::object_limit_fixture,
    );
    assert_atomic_cases(
        &runner,
        &temporary,
        "syntax",
        "baseline",
        &manifest.atomic_syntax_cases,
        common::syntax_fixture,
    );
    assert_checked_in_mutations(&runner, &temporary, &manifest.checked_in_mutations);
    assert_blend_mode_upstream_repro(&runner, &temporary);
    fs::remove_dir_all(temporary).expect("remove atomic fixture directory");
}

fn assert_checked_in_mutations(
    runner: &DifferentialRunner,
    temporary: &Path,
    cases: &[CheckedInMutation],
) {
    let mut baseline_paths = BTreeMap::<String, PathBuf>::new();
    let mut paths = Vec::with_capacity(cases.len() * 2);
    for case in cases {
        let baseline_path = baseline_paths
            .entry(case.family.clone())
            .or_insert_with(|| {
                let path = temporary.join(format!("shared-baseline-{}.pdf", case.family));
                fs::write(
                    &path,
                    shared_mutation_fixture(&case.family, &case.baseline_case),
                )
                .expect("write shared mutation baseline");
                path
            })
            .clone();
        paths.push(baseline_path);
        paths.push(case.path.clone());
    }
    let reports = runner.compare_files(&paths, &SafetyLimits::default());
    for (case, pair) in cases.iter().zip(reports.chunks_exact(2)) {
        let baseline = &pair[0];
        let mutation = &pair[1];
        assert!(
            matches!(
                baseline.classification,
                ComparisonClassification::Agreement
                    | ComparisonClassification::BothNoncompliant
                    | ComparisonClassification::CoverageGap
            ),
            "{}: shared mutation baseline has an unexpected classification: {baseline}",
            case.path.display()
        );
        assert!(
            matches!(
                mutation.classification,
                ComparisonClassification::BothNoncompliant | ComparisonClassification::CoverageGap
            ),
            "{} ({}): shared mutation classification: {mutation}",
            case.path.display(),
            case.rationale
        );
        let baseline_local_ids = baseline
            .local_report
            .failures
            .iter()
            .map(|failure| failure.rule_id.to_owned())
            .collect::<BTreeSet<_>>();
        let mutation_local_ids = mutation
            .local_report
            .failures
            .iter()
            .map(|failure| failure.rule_id.to_owned())
            .collect::<BTreeSet<_>>();
        assert!(
            case.expected_local_failed_rule_ids
                .contains(&case.local_rule_id),
            "{} does not declare its selected local rule",
            case.path.display()
        );
        assert_rule_id_delta(
            "local",
            &case.case,
            &case.rationale,
            &baseline_local_ids,
            &mutation_local_ids,
            &case.expected_local_failed_rule_ids,
            &case.expected_local_passed_rule_ids,
        );
        let baseline_reference_ids = baseline
            .reference_result
            .as_ref()
            .expect("shared mutation baseline veraPDF result")
            .failed_rule_ids
            .iter()
            .filter(|id| !is_untracked_1302_rule(id))
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        let mutation_reference_ids = mutation
            .reference_result
            .as_ref()
            .expect("shared mutation veraPDF result")
            .failed_rule_ids
            .iter()
            .filter(|id| !is_untracked_1302_rule(id))
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        assert!(
            case.expected_verapdf_failed_rule_ids
                .contains(&case.reference_rule_id),
            "{} does not declare its selected veraPDF rule",
            case.path.display()
        );
        assert_rule_id_delta(
            "veraPDF",
            &case.case,
            &case.rationale,
            &baseline_reference_ids,
            &mutation_reference_ids,
            &case.expected_verapdf_failed_rule_ids,
            &case.expected_verapdf_passed_rule_ids,
        );
    }
}

#[test]
fn pinned_verapdf_pdfa_1a_manifest_matches_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        eprintln!("VERAPDF_BIN is unset; skipping opt-in veraPDF differential run");
        return;
    };
    let manifest_path = Path::new("tests/fixtures/verapdf-diff-cases-1a.json");
    let manifest: PdfA1aManifest = serde_json::from_slice(
        &fs::read(manifest_path).expect("read PDF/A-1a differential case manifest"),
    )
    .expect("parse PDF/A-1a differential case manifest");
    assert_eq!(manifest.reference.version, PINNED_VERAPDF_VERSION);
    assert_eq!(manifest.reference.profile, "1a");
    let expected_a_only_rules = BTreeSet::from([
        "PDFA1A-ID-CONFORMANCE-001",
        "PDFA1A-TAGGED-DOCUMENT-001",
        "PDFA1A-STRUCT-TREE-ROOT-001",
        "PDFA1A-STRUCT-TREE-ROLE-MAP-001",
        "PDFA1A-STRUCT-TREE-ROLE-MAP-CYCLE-001",
        "PDFA1A-LANG-001",
        "PDFA1A-UNICODE-MAPPING-001",
    ]);
    let actual_a_only_rules = manifest
        .a_only_rules
        .iter()
        .map(|rule| rule.local_rule_id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_a_only_rules, expected_a_only_rules);

    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfA1a;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");

    let shared_paths = manifest
        .shared_cases
        .iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    for (path, report) in shared_paths
        .iter()
        .zip(runner.compare_files(&shared_paths, &SafetyLimits::default()))
    {
        assert!(
            matches!(
                report.classification,
                ComparisonClassification::Agreement | ComparisonClassification::BothNoncompliant
            ),
            "{}: unexpected PDF/A-1a classification: {report}",
            path.display()
        );
    }
    for path in &manifest.composition_cases {
        let report = runner.compare_file(path, &SafetyLimits::default());
        assert_eq!(
            report.classification,
            ComparisonClassification::Agreement,
            "{}: composition fixture must be a strict positive agreement: {report}",
            path.display()
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
    for case in &manifest.mutation_cases {
        let paths = vec![case.baseline.clone(), case.path.clone()];
        let mut reports = runner
            .compare_files(&paths, &SafetyLimits::default())
            .into_iter();
        let baseline = reports.next().expect("mutation baseline report");
        let mutation = reports.next().expect("mutation report");
        assert_eq!(
            baseline.classification,
            ComparisonClassification::Agreement,
            "{}: mutation baseline must be compliant: {baseline}",
            case.path.display()
        );
        assert_eq!(
            mutation.classification,
            ComparisonClassification::BothNoncompliant,
            "{}: mutation must have a semantic failure: {}",
            case.path.display(),
            mutation
        );
        let baseline_local_ids = baseline
            .local_report
            .failures
            .iter()
            .map(|failure| failure.rule_id.to_owned())
            .collect::<BTreeSet<_>>();
        let mutation_local_ids = mutation
            .local_report
            .failures
            .iter()
            .map(|failure| failure.rule_id.to_owned())
            .collect::<BTreeSet<_>>();
        assert_rule_id_delta(
            "local",
            &case.path.display().to_string(),
            &case.rationale,
            &baseline_local_ids,
            &mutation_local_ids,
            std::slice::from_ref(&case.local_rule_id),
            &[],
        );
        let baseline_reference_ids = baseline
            .reference_result
            .as_ref()
            .expect("mutation baseline veraPDF result")
            .failed_rule_ids
            .iter()
            .filter(|id| !is_untracked_1302_rule(id))
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        let mutation_reference_ids = mutation
            .reference_result
            .as_ref()
            .expect("mutation veraPDF result")
            .failed_rule_ids
            .iter()
            .filter(|id| !is_untracked_1302_rule(id))
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        assert_rule_id_delta(
            "veraPDF",
            &case.path.display().to_string(),
            &case.rationale,
            &baseline_reference_ids,
            &mutation_reference_ids,
            std::slice::from_ref(&case.reference_rule_id),
            &[],
        );
    }
    for case in &manifest.applicability_cases {
        let report = runner.compare_file(&case.path, &SafetyLimits::default());
        assert_eq!(
            report.classification,
            ComparisonClassification::Agreement,
            "{} ({}): applicability case must remain compliant: {report}",
            case.name,
            case.rationale
        );
        assert!(
            !report
                .local_report
                .failures
                .iter()
                .any(|failure| failure.rule_id == case.local_rule_id),
            "{}: inapplicable local rule {} unexpectedly failed: {report}",
            case.name,
            case.local_rule_id
        );
        assert!(
            !report
                .reference_result
                .as_ref()
                .expect("applicability veraPDF result")
                .failed_rule_ids
                .iter()
                .any(|rule| rule.to_string() == case.reference_rule_id),
            "{}: inapplicable veraPDF rule {} unexpectedly failed: {report}",
            case.name,
            case.reference_rule_id
        );
    }

    let temporary = env::temp_dir().join(format!("page-verapdf-pdfa-1a-{}", std::process::id()));
    fs::create_dir_all(&temporary).expect("create PDF/A-1a differential fixture directory");
    for rule in &manifest.a_only_rules {
        assert!(
            !rule.cases.is_empty(),
            "{} has no matrix cases",
            rule.local_rule_id
        );
        let statuses = rule
            .cases
            .iter()
            .map(|case| case.status.as_str())
            .collect::<BTreeSet<_>>();
        assert!(
            statuses.contains("pass"),
            "{} has no pass case",
            rule.local_rule_id
        );
        assert!(
            statuses.contains("fail"),
            "{} has no fail case",
            rule.local_rule_id
        );
        let mut case_names = BTreeSet::new();
        for case in &rule.cases {
            assert!(
                case_names.insert(case.name.as_str()),
                "{} has duplicate matrix case {}",
                rule.local_rule_id,
                case.name
            );
        }
        for case in &rule.cases {
            assert!(
                matches!(case.status.as_str(), "pass" | "fail" | "inapplicable"),
                "{}: unsupported matrix status {}",
                case.name,
                case.status
            );
            let expected_failure = case.status == "fail";
            assert_eq!(
                case.expected_local_failure, expected_failure,
                "{}: status/local expectation mismatch",
                case.name
            );
            assert_eq!(
                case.expected_verapdf_failure, expected_failure,
                "{}: status/veraPDF expectation mismatch",
                case.name
            );
            let bytes = fixture_bytes(&case.fixture_family, &case.name);
            let path = temporary.join(format!("{}-{}.pdf", rule.local_rule_id, case.name));
            fs::write(&path, &bytes).expect("write PDF/A-1a differential fixture");
            let report = runner.compare_file(&path, &SafetyLimits::default());
            assert!(
                matches!(
                    report.classification,
                    ComparisonClassification::Agreement
                        | ComparisonClassification::BothNoncompliant
                ),
                "{}: unexpected PDF/A-1a classification: {report}",
                case.rationale
            );
            let local_failed = report
                .local_report
                .failures
                .iter()
                .any(|failure| failure.rule_id == rule.local_rule_id);
            assert_eq!(
                local_failed, case.expected_local_failure,
                "{}: local rule {} expectation failed: {report}",
                case.rationale, rule.local_rule_id
            );
            let reference = report.reference_result.as_ref().expect("veraPDF result");
            let reference_failed = reference
                .failed_rule_ids
                .iter()
                .any(|id| id.to_string() == rule.reference_rule_id);
            assert_eq!(
                reference_failed, case.expected_verapdf_failure,
                "{}: veraPDF rule {} expectation failed: {report}",
                case.rationale, rule.reference_rule_id
            );
        }
    }
    for case in &manifest.upstream_failure_cases {
        let bytes = fixture_bytes(&case.fixture_family, &case.name);
        let path = temporary.join(format!("{}-upstream.pdf", case.name));
        fs::write(&path, &bytes).expect("write upstream failure fixture");
        let report = runner.compare_file(&path, &SafetyLimits::default());
        assert_eq!(
            report.classification,
            ComparisonClassification::Operational,
            "{}: expected veraPDF upstream failure: {report}",
            case.rationale
        );
        assert!(
            report
                .local_report
                .failures
                .iter()
                .any(|failure| failure.rule_id == case.expected_local_rule_id),
            "{}: local rule {} did not fail: {}",
            case.rationale,
            case.expected_local_rule_id,
            case.name
        );
    }
    for case in &manifest.upstream_mismatch_cases {
        let bytes = fixture_bytes(&case.fixture_family, &case.name);
        let path = temporary.join(format!("{}-mismatch.pdf", case.name));
        fs::write(&path, &bytes).expect("write upstream mismatch fixture");
        let report = runner.compare_file(&path, &SafetyLimits::default());
        assert_eq!(
            report.classification, case.expected_classification,
            "{}: unexpected classification: {report}",
            case.rationale
        );
        let local_failed = report
            .local_report
            .failures
            .iter()
            .any(|failure| failure.rule_id == case.local_rule_id);
        assert_eq!(
            local_failed, case.expected_local_failure,
            "{}: {report}",
            case.rationale
        );
        let reference = report.reference_result.as_ref().expect("veraPDF result");
        let reference_failed = reference
            .failed_rule_ids
            .iter()
            .any(|id| id.to_string() == case.reference_rule_id);
        assert_eq!(
            reference_failed, case.expected_verapdf_failure,
            "{}: {report}",
            case.rationale
        );
    }
    fs::remove_dir_all(temporary).expect("remove PDF/A-1a differential fixture directory");
}

fn fixture_bytes(fixture_family: &str, name: &str) -> Vec<u8> {
    match fixture_family {
        "metadata" => common::metadata_fixture(name),
        "tagged_document" => common::tagged_document_fixture(name),
        "font" => common::font_fixture(name),
        family => panic!("{name}: unsupported fixture family {family}"),
    }
}

fn assert_blend_mode_upstream_repro(runner: &DifferentialRunner, temporary: &Path) {
    let path = temporary.join("transparency-extgstate-bm-multiply.pdf");
    fs::write(&path, common::graphics_fixture("extgstate_bm_multiply"))
        .expect("write blend-mode upstream repro");
    let report = runner.compare_file(&path, &SafetyLimits::default());
    assert!(
        report
            .local_report
            .failures
            .iter()
            .any(|failure| failure.rule_id == "PDFA1B-EXTGSTATE-BLEND-MODE-001"),
        "the minimal repro must exercise the local blend-mode predicate"
    );
    assert_eq!(
        report.classification,
        ComparisonClassification::BothNoncompliant
    );
    assert!(
        report
            .reference_result
            .as_ref()
            .expect("veraPDF result")
            .failed_rule_ids
            .iter()
            .any(|rule| rule.to_string() == "ISO 19005-1:2005:6.4:4")
    );
}

fn assert_atomic_cases(
    runner: &DifferentialRunner,
    temporary: &Path,
    prefix: &str,
    baseline_name: &str,
    cases: &[AtomicRuleCase],
    fixture: fn(&str) -> Vec<u8>,
) {
    let filter = std::env::var("PAGE_ATOMIC_FILTER").ok();
    if filter
        .as_deref()
        .and_then(|filter| filter.split_once(':'))
        .is_some_and(|(selected_prefix, _)| selected_prefix != prefix)
    {
        return;
    }
    let selected_cases = cases
        .iter()
        .filter(|case| {
            !filter
                .as_deref()
                .and_then(|filter| filter.split_once(':'))
                .is_some_and(|(_, selected_case)| {
                    selected_case != "*" && selected_case != case.name
                })
        })
        .collect::<Vec<_>>();
    if selected_cases.is_empty() {
        return;
    }
    let baseline_path = temporary.join(format!("{prefix}-baseline.pdf"));
    fs::write(&baseline_path, fixture(baseline_name)).expect("write baseline PDF");
    let mut paths = Vec::with_capacity(selected_cases.len() + 1);
    paths.push(baseline_path);
    for case in &selected_cases {
        let path = temporary.join(format!("{prefix}-{}.pdf", case.name));
        fs::write(&path, fixture(&case.name)).expect("write atomic PDF");
        paths.push(path);
    }
    let mut reports = runner
        .compare_files(&paths, &SafetyLimits::default())
        .into_iter();
    let baseline = reports.next().expect("baseline report");
    let baseline_local_ids = baseline
        .local_report
        .failures
        .iter()
        .map(|failure| failure.rule_id.to_owned())
        .collect::<BTreeSet<_>>();
    let baseline_reference_ids = baseline
        .reference_result
        .as_ref()
        .expect("baseline reference result")
        .failed_rule_ids
        .iter()
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>();
    for (case, report) in selected_cases.into_iter().zip(reports) {
        let local_ids = report
            .local_report
            .failures
            .iter()
            .map(|failure| failure.rule_id.to_owned())
            .collect::<BTreeSet<_>>();
        assert_rule_id_delta(
            "local",
            &case.name,
            &case.rationale,
            &baseline_local_ids,
            &local_ids,
            &case.expected_local_failed_rule_ids,
            &case.expected_local_passed_rule_ids,
        );

        let reference = report
            .reference_result
            .as_ref()
            .unwrap_or_else(|| panic!("{}: reference failed: {report}", case.name));
        let reference_ids = reference
            .failed_rule_ids
            .iter()
            .filter(|id| !is_untracked_1302_rule(id))
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        assert_rule_id_delta(
            "veraPDF",
            &case.name,
            &case.rationale,
            &baseline_reference_ids,
            &reference_ids,
            &case.expected_verapdf_failed_rule_ids,
            &case.expected_verapdf_passed_rule_ids,
        );
    }
}

// veraPDF 1.30.2 adds PDF/A-1 metadata predicates 6.7.3:1 and 6.7.3:8.
// The repository's pinned 129-predicate profile intentionally remains the
// legacy profile, so these upstream-only deltas are outside this manifest.
fn is_untracked_1302_rule(id: &page_validation::differential::ReferenceRuleId) -> bool {
    id.specification == "ISO 19005-1:2005"
        && id.clause == "6.7.3"
        && matches!(id.test_number, 1 | 8)
}

fn is_untracked_1302_rule_string(id: &str) -> bool {
    matches!(id, "ISO 19005-1:2005:6.7.3:1" | "ISO 19005-1:2005:6.7.3:8")
}

/// Asserts that `actual_ids` adds exactly `expected_added`, removes only
/// failures explicitly named by `expected_passed`, and contains none of the
/// expected passing IDs.
fn assert_rule_id_delta(
    label: &str,
    case_name: &str,
    rationale: &str,
    baseline_ids: &BTreeSet<String>,
    actual_ids: &BTreeSet<String>,
    expected_added: &[String],
    expected_passed: &[String],
) {
    let (added, removed) = common::rule_delta(baseline_ids, actual_ids);
    let expected = expected_added
        .iter()
        .filter(|id| !is_untracked_1302_rule_string(id))
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        added, expected,
        "{case_name}: {rationale}: unexpected {label} rule delta; full set {actual_ids:?}"
    );
    let expected_passed = expected_passed
        .iter()
        .filter(|id| !is_untracked_1302_rule_string(id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let unexpected_removed = removed
        .difference(&expected_passed)
        .cloned()
        .collect::<BTreeSet<_>>();
    assert!(
        unexpected_removed.is_empty(),
        "{case_name}: {rationale}: unexpectedly removed baseline {label} failures \
         {unexpected_removed:?}"
    );
    for expected in expected_passed {
        assert!(
            !actual_ids.contains(&expected),
            "{case_name}: {rationale}: unexpected {label} failure {expected}; actual {actual_ids:?}"
        );
    }
}
