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

const PROFILES: [&str; 6] = [
    "PDF_A-1a", "PDF_A-1b", "PDF_A-2a", "PDF_A-2b", "PDF_A-2u", "PDF_A-3b",
];

fn corpus_with_parse_failures() -> TempDirectory {
    let temporary = TempDirectory::new();
    for profile in PROFILES {
        let directory = temporary.join(profile);
        fs::create_dir(&directory).expect("create corpus profile directory");
        fs::write(
            directory.join("veraPDF test suite 6-1-t01-fail-a.pdf"),
            b"not a PDF",
        )
        .expect("write corpus failure case");
    }
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

    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .args(["corpus", temporary.0.to_str().expect("UTF-8 corpus path")])
        .output()
        .expect("run corpus command");

    assert_eq!(output.status.code(), Some(0));
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 corpus output");
    assert!(stderr.contains("Corpus validation"), "{stderr}");
    assert!(stderr.contains("cases:       7"), "{stderr}");
    assert!(stderr.contains("matched:     7"), "{stderr}");
}

#[test]
fn corpus_command_returns_two_for_an_expected_pass_failure() {
    let temporary = corpus_with_parse_failures();
    fs::write(
        temporary.join("PDF_A-1b/veraPDF test suite 6-1-t01-fail-a.pdf"),
        include_bytes!("../../page_validation/tests/fixtures/canonical-pdfa-1b.pdf"),
    )
    .expect("replace corpus failure case");

    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .args(["corpus", temporary.0.to_str().expect("UTF-8 corpus path")])
        .output()
        .expect("run corpus command");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 corpus output");
    assert!(stderr.contains("Corpus mismatch"), "{stderr}");
    assert!(stderr.contains("mismatches:  1"), "{stderr}");
    assert!(stderr.contains("result:      fail (exit 2)"), "{stderr}");
}
