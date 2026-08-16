use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::PathBuf;

use page_validation::SafetyLimits;
use page_validation::differential::{
    ComparisonClassification, CoverageGapPolicy, DifferentialRunner, ReferenceConfig,
    ReferenceProfile,
};
use serde::Deserialize;

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
