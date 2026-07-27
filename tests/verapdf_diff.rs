use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use pdf::SafetyLimits;
use pdf::differential::{
    ComparisonClassification, DifferentialRunner, PINNED_VERAPDF_PROFILE, PINNED_VERAPDF_VERSION,
    ReferenceConfig,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Manifest {
    reference: ManifestReference,
    cases: Vec<ManifestCase>,
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
}
