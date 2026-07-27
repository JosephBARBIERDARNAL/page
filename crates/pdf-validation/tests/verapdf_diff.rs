use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use pdf_validation::SafetyLimits;
use pdf_validation::differential::{
    ComparisonClassification, DifferentialRunner, PINNED_VERAPDF_PROFILE, PINNED_VERAPDF_VERSION,
    ReferenceConfig,
};
use serde::Deserialize;

mod common;

#[derive(Debug, Deserialize)]
struct Manifest {
    reference: ManifestReference,
    cases: Vec<ManifestCase>,
    #[serde(default)]
    atomic_metadata_cases: Vec<AtomicRuleCase>,
    #[serde(default)]
    atomic_output_intent_cases: Vec<AtomicRuleCase>,
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
    for case in manifest.cases {
        let report = runner.compare_file(&case.path, &SafetyLimits::default());
        assert_eq!(
            report.classification,
            case.expected_classification,
            "{}: {}\n{report}",
            case.path.display(),
            case.rationale
        );
    }

    let temporary = std::env::temp_dir().join(format!(
        "pdf-verapdf-metadata-atomic-{}",
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
    fs::remove_dir_all(temporary).expect("remove atomic fixture directory");
}

fn assert_atomic_cases(
    runner: &DifferentialRunner,
    temporary: &Path,
    prefix: &str,
    baseline_name: &str,
    cases: &[AtomicRuleCase],
    fixture: fn(&str) -> Vec<u8>,
) {
    let baseline_path = temporary.join(format!("{prefix}-baseline.pdf"));
    fs::write(&baseline_path, fixture(baseline_name)).expect("write baseline PDF");
    let baseline = runner.compare_file(&baseline_path, &SafetyLimits::default());
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
    for case in cases {
        let path = temporary.join(format!("{prefix}-{}.pdf", case.name));
        fs::write(&path, fixture(&case.name)).expect("write atomic PDF");
        let report = runner.compare_file(&path, &SafetyLimits::default());
        let local_ids = report
            .local_report
            .failures
            .iter()
            .map(|failure| failure.rule_id)
            .collect::<Vec<_>>();
        let local_delta = local_ids
            .iter()
            .filter(|id| !baseline_local_ids.contains(**id))
            .map(|id| (*id).to_owned())
            .collect::<BTreeSet<_>>();
        let expected = case
            .expected_local_failed_rule_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            local_delta, expected,
            "{}: {}: unexpected local rule delta; full set {local_ids:?}",
            case.name, case.rationale
        );
        let removed_local = baseline_local_ids
            .iter()
            .filter(|id| !local_ids.contains(&id.as_str()))
            .cloned()
            .collect::<BTreeSet<_>>();
        assert!(
            removed_local.is_empty(),
            "{}: {}: unexpectedly removed baseline local failures {removed_local:?}",
            case.name,
            case.rationale
        );
        for expected in &case.expected_local_passed_rule_ids {
            assert!(
                !local_ids.contains(&expected.as_str()),
                "{}: {}: unexpected local failure {expected}; actual {local_ids:?}",
                case.name,
                case.rationale
            );
        }
        let reference = report
            .reference_result
            .as_ref()
            .unwrap_or_else(|| panic!("{}: reference failed: {report}", case.name));
        let reference_ids = reference
            .failed_rule_ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let reference_delta = reference_ids
            .iter()
            .filter(|id| !baseline_reference_ids.contains(*id))
            .cloned()
            .collect::<BTreeSet<_>>();
        let expected = case
            .expected_verapdf_failed_rule_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            reference_delta, expected,
            "{}: {}: unexpected veraPDF rule delta; full set {reference_ids:?}",
            case.name, case.rationale
        );
        let removed_reference = baseline_reference_ids
            .iter()
            .filter(|id| !reference_ids.contains(id))
            .cloned()
            .collect::<BTreeSet<_>>();
        assert!(
            removed_reference.is_empty(),
            "{}: {}: unexpectedly removed baseline veraPDF failures {removed_reference:?}",
            case.name,
            case.rationale
        );
        for expected in &case.expected_verapdf_passed_rule_ids {
            assert!(
                !reference_ids.contains(expected),
                "{}: {}: unexpected veraPDF failure {expected}; actual {reference_ids:?}",
                case.name,
                case.rationale
            );
        }
    }
}
