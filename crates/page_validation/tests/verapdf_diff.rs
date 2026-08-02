use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use page_validation::SafetyLimits;
use page_validation::differential::{
    ComparisonClassification, DifferentialRunner, OperationalFailureKind, PINNED_VERAPDF_PROFILE,
    PINNED_VERAPDF_VERSION, ReferenceConfig,
};
use serde::Deserialize;

#[allow(dead_code)]
mod common;

#[derive(Debug, Deserialize)]
struct Manifest {
    reference: ManifestReference,
    cases: Vec<ManifestCase>,
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
    assert_blend_mode_upstream_repro(&runner, &temporary);
    fs::remove_dir_all(temporary).expect("remove atomic fixture directory");
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
    assert_eq!(report.classification, ComparisonClassification::Operational);
    let failure = report
        .operational_failure
        .expect("veraPDF VALIDATE exception must be reported operationally");
    assert_eq!(failure.kind, OperationalFailureKind::InvalidReferenceReport);
    assert!(
        failure.message.contains("VALIDATE"),
        "unexpected veraPDF failure for blend-mode repro: {}",
        failure.message
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

/// Asserts that `actual_ids` adds exactly `expected_added`, removes only
/// failures explicitly named by `expected_passed`, and contains none of the
/// expected passing IDs.
#[allow(clippy::too_many_arguments)]
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
    let expected = expected_added.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(
        added, expected,
        "{case_name}: {rationale}: unexpected {label} rule delta; full set {actual_ids:?}"
    );
    let expected_passed = expected_passed.iter().cloned().collect::<BTreeSet<_>>();
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
