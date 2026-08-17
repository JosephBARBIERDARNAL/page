use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let sequence = TEMP_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("page-cli-tests-{}-{sequence}", std::process::id()));
        fs::create_dir(&path).expect("create temporary CLI test directory");
        Self(path)
    }

    fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        self.0.join(path)
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove temporary CLI test directory");
    }
}

fn noncompliant_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../page_validation/tests/fixtures/trailer-id-missing.pdf")
}

#[test]
fn page_help_exposes_the_validate_command() {
    let root = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg("--help")
        .output()
        .expect("run page --help");

    assert!(root.status.success());
    let root_stdout = String::from_utf8(root.stdout).expect("UTF-8 root help");
    assert!(root_stdout.contains("Usage: page <COMMAND>"));
    assert!(root_stdout.contains("validate"));
    assert!(root_stdout.contains("corpus"));

    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .args(["validate", "--help"])
        .output()
        .expect("run page validate --help");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 help");
    assert!(stdout.contains("Usage: page validate [OPTIONS] <FILE>"));
    assert!(stdout.contains("--format <FORMAT>"));
    assert!(stdout.contains("details, json"));
    assert!(stdout.contains("--output <FILE>"));
    assert!(stdout.contains("--no-color"));
    assert!(!stdout.contains("--json"));
    assert!(
        stdout.contains(
            "a-1b, a-1a, a-2b, a-2a, a-2u, a-3b, a-3a, a-3u, a-4, a-4e, a-4f, ua-1, ua-2"
        )
    );
}

#[test]
fn json_extension_infers_json_file_output() {
    let temporary = TempDirectory::new();
    let report_path = temporary.join("report.JSON");
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg("validate")
        .arg(noncompliant_fixture())
        .args(["--output", report_path.to_str().expect("UTF-8 report path")])
        .output()
        .expect("write inferred JSON report");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    let contents = fs::read_to_string(report_path).expect("read JSON report");
    assert!(contents.ends_with('\n'));
    let report: serde_json::Value = serde_json::from_str(&contents).expect("parse JSON report");
    assert_eq!(report["valid"], false);
}

#[test]
fn txt_extension_writes_plain_summary_and_replaces_existing_output() {
    let temporary = TempDirectory::new();
    let report_path = temporary.join("report.txt");
    fs::write(&report_path, "stale report").expect("seed existing report");
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg("validate")
        .arg(noncompliant_fixture())
        .args(["--output", report_path.to_str().expect("UTF-8 report path")])
        .output()
        .expect("write text report");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    let contents = fs::read_to_string(report_path).expect("read text report");
    assert!(contents.starts_with("Profile : PDF/A-1b\nResult  : Non-conformant\n"));
    assert!(contents.contains("✓ PDF syntax\n"));
    assert!(contents.contains("\nTime    : "));
    assert!(!contents.contains('\u{1b}'));
    assert_ne!(contents, "stale report");
}

#[test]
fn explicit_json_format_allows_an_extensionless_output() {
    let temporary = TempDirectory::new();
    let report_path = temporary.join("report");
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg("validate")
        .arg(noncompliant_fixture())
        .args([
            "--format",
            "json",
            "--output",
            report_path.to_str().expect("UTF-8 report path"),
        ])
        .output()
        .expect("write extensionless JSON report");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    let contents = fs::read(report_path).expect("read extensionless report");
    serde_json::from_slice::<serde_json::Value>(&contents)
        .expect("parse extensionless JSON report");
}

#[test]
fn recognized_extensions_reject_a_conflicting_explicit_format() {
    let temporary = TempDirectory::new();
    let cases = [
        ("json", "report.txt", "'.txt' conflicts with JSON format"),
        (
            "details",
            "report.json",
            "'.json' conflicts with details format",
        ),
    ];

    for (format, file_name, expected_error) in cases {
        let report_path = temporary.join(file_name);
        let output = Command::new(env!("CARGO_BIN_EXE_page"))
            .arg("validate")
            .arg(noncompliant_fixture())
            .args([
                "--format",
                format,
                "--output",
                report_path.to_str().expect("UTF-8 report path"),
            ])
            .output()
            .expect("reject conflicting output extension");

        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .expect("UTF-8 conflict error")
                .contains(expected_error)
        );
        assert!(!report_path.exists());
    }
}

#[test]
fn output_cannot_replace_the_input_pdf() {
    let fixture = noncompliant_fixture();
    let original = fs::read(&fixture).expect("read fixture before validation");
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg("validate")
        .arg(&fixture)
        .args(["--output", fixture.to_str().expect("UTF-8 fixture path")])
        .output()
        .expect("reject input as output");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).expect("UTF-8 same-file error"),
        "error: input and output paths refer to the same file\n"
    );
    assert_eq!(
        fs::read(fixture).expect("read fixture after validation"),
        original
    );
}

#[test]
fn default_validation_output_is_a_compact_summary() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../page_validation/tests/fixtures/trailer-id-missing.pdf");
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg("validate")
        .arg(&fixture)
        .output()
        .expect("run PDF validation");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let summary = String::from_utf8(output.stdout).expect("UTF-8 summary");
    assert!(summary.starts_with("Profile : PDF/A-1b\nResult  : Non-conformant\n\n"));
    assert!(summary.contains("✓ PDF syntax\n"));
    assert!(summary.contains("✓ PDF/A-1b\n"));
    assert!(summary.contains("✓ Metadata\n"));
    assert!(summary.contains("✓ Color\n"));
    assert!(summary.contains("✓ Fonts\n"));
    assert!(summary.contains("✓ Images\n"));
    assert!(summary.contains("✓ Graphics\n"));
    assert!(summary.contains("✓ Interactive content\n"));
    assert!(summary.contains("✗ Structure\n"));
    assert!(summary.contains("\nTime    : "));
    assert!(summary.ends_with("s\n"));
    assert!(!summary.contains('['));
}

#[test]
fn missing_declared_profile_is_an_explicit_error() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../page_validation/tests/fixtures/structural.pdf");
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg("validate")
        .arg(&fixture)
        .output()
        .expect("run PDF validation without a declared profile");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).expect("UTF-8 error"),
        "error: document does not declare a PDF/A or PDF/UA validation profile, declare it with --profile\n"
    );
}

#[test]
fn no_color_flag_preserves_plain_human_output() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../page_validation/tests/fixtures/structural.pdf");
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg("validate")
        .arg(&fixture)
        .args(["--profile", "a-1b", "--format", "details", "--no-color"])
        .output()
        .expect("run PDF validation without colors");

    assert_eq!(output.status.code(), Some(2));
    assert!(!output.stdout.contains(&0x1b));
    assert!(!output.stderr.contains(&0x1b));
}

#[test]
fn details_format_prints_every_failed_rule() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../page_validation/tests/fixtures/structural.pdf");
    let details = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg("validate")
        .arg(&fixture)
        .args(["--profile", "a-1b", "--format", "details"])
        .output()
        .expect("run detailed PDF validation");
    let json = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg("validate")
        .arg(&fixture)
        .args(["--profile", "a-1b", "--format", "json"])
        .output()
        .expect("run JSON PDF validation");

    assert_eq!(details.status.code(), Some(2));
    assert!(details.stderr.is_empty());
    let details = String::from_utf8(details.stdout).expect("UTF-8 details");
    assert!(details.starts_with("Profile : PDF/A-1b\nResult  : Non-conformant\n\n"));
    let time = details.find("Time    : ").expect("summary duration");
    let checks = details.find("Checks: ").expect("detailed check counts");
    let first_failure = details.find('[').expect("first detailed failure");
    assert!(time < checks);
    assert!(checks < first_failure);
    let detailed_failure_count = details.lines().filter(|line| line.starts_with('[')).count();
    let json: serde_json::Value = serde_json::from_slice(&json.stdout).expect("validation JSON");
    assert_eq!(
        detailed_failure_count,
        json["failures"].as_array().expect("JSON failures").len()
    );
    assert!(detailed_failure_count > 0);
}

#[test]
fn remaining_profiles_are_recognized_and_reported_as_unimplemented() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../page_validation/tests/fixtures/structural.pdf");
    let profiles = [
        ("a-4", "PDF/A-4"),
        ("a-4e", "PDF/A-4e"),
        ("a-4f", "PDF/A-4f"),
        ("ua-1", "PDF/UA-1"),
        ("ua-2", "PDF/UA-2"),
    ];

    for (argument, display_name) in profiles {
        let output = Command::new(env!("CARGO_BIN_EXE_page"))
            .arg("validate")
            .arg(&fixture)
            .args(["--profile", argument])
            .output()
            .expect("run validation with a future profile");

        assert_eq!(output.status.code(), Some(1), "profile {argument}");
        assert!(output.stdout.is_empty(), "profile {argument}");
        assert_eq!(
            String::from_utf8(output.stderr).expect("UTF-8 error"),
            format!("error: validation profile {display_name} is not implemented yet\n")
        );
    }
}

#[test]
fn validation_json_uses_the_stable_public_schema() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../page_validation/tests/fixtures/trailer-id-missing.pdf");
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg("validate")
        .arg(&fixture)
        .args(["--format", "json"])
        .output()
        .expect("run PDF validation");

    assert_eq!(output.status.code(), Some(2));
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("validation JSON report");
    assert_eq!(report["file"], fixture.display().to_string());
    assert_eq!(report["profile"], "a-1b");
    assert_eq!(report["valid"], false);
    assert!(report["error"].is_null());
    assert!(
        report["failures"]
            .as_array()
            .is_some_and(|failures| !failures.is_empty())
    );
    assert!(report["failures"][0]["rule"].is_string());
    assert!(report["failures"][0]["message"].is_string());
}

#[test]
fn validation_json_reports_parser_errors_separately() {
    let validation = Path::new(env!("CARGO_MANIFEST_DIR")).join("../page_validation");
    let malformed = validation.join("tests/fixtures/malformed.pdf");
    let parser = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg("validate")
        .arg(malformed)
        .args(["--profile", "a-1b", "--format", "json"])
        .output()
        .expect("run malformed PDF validation");
    assert_eq!(parser.status.code(), Some(2));
    let parser: serde_json::Value = serde_json::from_slice(&parser.stdout).expect("parser JSON");
    assert_eq!(parser["valid"], false);
    assert_eq!(parser["failures"], serde_json::json!([]));
    assert_eq!(parser["error"]["kind"], "parser");
}

#[test]
fn missing_input_ignores_json_format_and_reports_a_direct_error() {
    let missing =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../page_validation/tests/fixtures/missing.pdf");
    let without_json = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg("validate")
        .arg(&missing)
        .args(["--profile", "a-1b"])
        .output()
        .expect("run missing PDF validation");
    let with_json = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg("validate")
        .arg(&missing)
        .args(["--profile", "a-1b", "--format", "json"])
        .output()
        .expect("run missing PDF validation with JSON format");

    assert_eq!(with_json.status.code(), Some(1));
    assert!(with_json.stdout.is_empty());
    let missing_file_error = std::io::Error::from_raw_os_error(2);
    assert_eq!(
        std::str::from_utf8(&with_json.stderr).expect("UTF-8 error"),
        format!(
            "error: could not read '{}': {missing_file_error}\n",
            missing.display(),
        )
    );
    assert_eq!(with_json.status, without_json.status);
    assert_eq!(with_json.stdout, without_json.stdout);
    assert_eq!(with_json.stderr, without_json.stderr);
}

#[test]
fn differential_help_keeps_its_own_client_contract() {
    let output = Command::new(env!("CARGO_BIN_EXE_verapdf-diff"))
        .arg("--help")
        .output()
        .expect("run verapdf-diff --help");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 help");
    assert!(stdout.contains("Usage: verapdf-diff"));
    assert!(stdout.contains("--verapdf"));
    assert!(stdout.contains("--expected-version"));
    assert!(stdout.contains("--batch-size"));
}
