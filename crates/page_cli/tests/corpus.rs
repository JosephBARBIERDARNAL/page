#![cfg(feature = "internal")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let sequence = TEMP_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "page-corpus-cli-tests-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create temporary corpus directory");
        Self(path)
    }

    fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        self.0.join(path)
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove temporary corpus directory");
    }
}

fn profiles() -> impl Iterator<Item = &'static str> {
    include_str!("../src/corpus_profiles.txt")
        .lines()
        .filter_map(|line| line.split_whitespace().next())
}

fn extra_sources() -> [(&'static str, &'static str); 4] {
    [
        ("ISO 32000-1", "veraPDF test suite 6-1-t01-fail-a.pdf"),
        (
            "Isartor test files/PDFA-1b",
            "veraPDF test suite 6-1-t01-fail-a.pdf",
        ),
        ("TWG test files", "TWG test suite A001-pdfa1-fail-a.pdf"),
        (
            "Undefined",
            "veraPDF test suite 6-2-3-2-t01-undefined-a.pdf",
        ),
    ]
}

fn corpus_with_parse_failures() -> TempDirectory {
    let temporary = TempDirectory::new();
    let mut rule_manifest = String::from(
        "# test rule expectations\n# profile directory<TAB>relative PDF path<TAB>reference rule<TAB>local rule\n",
    );
    for profile in profiles() {
        let directory = temporary.join(profile);
        fs::create_dir_all(&directory).expect("create corpus profile directory");
        fs::write(
            directory.join("veraPDF test suite 6-1-t01-fail-a.pdf"),
            b"not a PDF",
        )
        .expect("write corpus failure case");
        rule_manifest.push_str(&format!(
            "{profile}\tveraPDF test suite 6-1-t01-fail-a.pdf\tPDF-PARSE-001\tPDF-PARSE-001\n"
        ));
    }
    for (directory, file_name) in extra_sources() {
        let directory = temporary.join(directory);
        fs::create_dir_all(&directory).expect("create extra corpus directory");
        fs::write(directory.join(file_name), b"not a PDF").expect("write extra corpus case");
        let manifest_directory = directory
            .strip_prefix(&temporary.0)
            .expect("extra directory relative to corpus root")
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/")
            .replace("/PDFA-1b", "");
        rule_manifest.push_str(&format!(
            "{manifest_directory}\t{file_name}\tPDF-PARSE-001\tPDF-PARSE-001\n"
        ));
    }
    fs::write(temporary.join("rule-expectations.tsv"), rule_manifest)
        .expect("write test rule expectation manifest");
    temporary
}

#[test]
fn corpus_command_accepts_all_expected_failures_and_a_pass() {
    let temporary = corpus_with_parse_failures();
    fs::write(
        temporary.join("PDF_A-1b/veraPDF test suite 6-1-t01-pass-a.pdf"),
        include_bytes!("../../page_validation/tests/fixtures/canonical-pdfa-1b.pdf"),
    )
    .expect("write corpus pass case");

    let output = Command::new(env!("CARGO_BIN_EXE_page-corpus"))
        .arg("--rule-manifest")
        .arg(temporary.join("rule-expectations.tsv"))
        .arg(temporary.0.to_str().expect("UTF-8 corpus path"))
        .output()
        .expect("run corpus command");

    assert_eq!(output.status.code(), Some(0));
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 corpus output");
    assert!(stderr.contains("Corpus validation"), "{stderr}");
    assert!(stderr.contains("cases:       12"), "{stderr}");
    assert!(stderr.contains("matched:     12"), "{stderr}");
}

#[test]
fn corpus_command_returns_two_for_an_expected_pass_failure() {
    let temporary = corpus_with_parse_failures();
    fs::write(
        temporary.join("PDF_A-1b/veraPDF test suite 6-1-t01-fail-a.pdf"),
        include_bytes!("../../page_validation/tests/fixtures/canonical-pdfa-1b.pdf"),
    )
    .expect("replace corpus failure case");

    let output = Command::new(env!("CARGO_BIN_EXE_page-corpus"))
        .arg("--rule-manifest")
        .arg(temporary.join("rule-expectations.tsv"))
        .arg(temporary.0.to_str().expect("UTF-8 corpus path"))
        .output()
        .expect("run corpus command");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 corpus output");
    assert!(stderr.contains("Corpus mismatch"), "{stderr}");
    assert!(stderr.contains("mismatches:  1"), "{stderr}");
    assert!(stderr.contains("result:      fail (exit 2)"), "{stderr}");
}
