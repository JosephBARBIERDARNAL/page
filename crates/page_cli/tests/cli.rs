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
fn page_help_exposes_direct_validation_arguments() {
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg("--help")
        .output()
        .expect("run page --help");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 help");
    assert!(stdout.contains("Usage: page [OPTIONS] <FILE>"));
    assert!(stdout.contains("--format <FORMAT>"));
    assert!(stdout.contains("details, json"));
    assert!(stdout.contains("--output <FILE>"));
    assert!(stdout.contains("--no-color"));
    assert!(stdout.contains("--disable-safety-limits"));
    assert!(stdout.contains("trusted files"));
    assert!(stdout.contains("--max-input-size <MAX_INPUT_SIZE>"));
    assert!(stdout.contains("--max-decoded-stream-size <MAX_DECODED_STREAM_SIZE>"));
    assert!(stdout.contains("--max-total-decoded-content-size <MAX_TOTAL_DECODED_CONTENT_SIZE>"));
    assert!(stdout.contains("--max-object-count <MAX_OBJECT_COUNT>"));
    assert!(stdout.contains("--max-reference-depth <MAX_REFERENCE_DEPTH>"));
    assert!(stdout.contains("--max-xref-revisions <MAX_XREF_REVISIONS>"));
    assert!(stdout.contains("--max-table-span <MAX_TABLE_SPAN>"));
    assert!(stdout.contains("--max-table-grid-rows <MAX_TABLE_GRID_ROWS>"));
    assert!(stdout.contains("--max-table-grid-columns <MAX_TABLE_GRID_COLUMNS>"));
    assert!(stdout.contains("--max-table-grid-cells <MAX_TABLE_GRID_CELLS>"));
    assert!(stdout.contains("--max-unicode-cmap-mappings <MAX_UNICODE_CMAP_MAPPINGS>"));
    assert!(!stdout.contains("--json"));
    assert!(stdout.contains("1b, 1a, 2b, 2a, 2u, 3b, 3a, 3u, ua1"));
}

#[test]
fn disabling_safety_limits_conflicts_with_every_explicit_limit() {
    let defaults = page_validation::SafetyLimits::default();
    for (flag, value) in [
        ("--max-input-size", defaults.max_input_size.to_string()),
        (
            "--max-decoded-stream-size",
            defaults.max_decoded_stream_size.to_string(),
        ),
        (
            "--max-total-decoded-content-size",
            defaults.max_total_decoded_content_size.to_string(),
        ),
        (
            "--max-form-invocations",
            defaults.max_form_invocations.to_string(),
        ),
        ("--max-object-count", defaults.max_object_count.to_string()),
        (
            "--max-reference-depth",
            defaults.max_reference_depth.to_string(),
        ),
        (
            "--max-xref-revisions",
            defaults.max_xref_revisions.to_string(),
        ),
        ("--max-table-span", defaults.max_table_span.to_string()),
        (
            "--max-table-grid-rows",
            defaults.max_table_grid_rows.to_string(),
        ),
        (
            "--max-table-grid-columns",
            defaults.max_table_grid_columns.to_string(),
        ),
        (
            "--max-table-grid-cells",
            defaults.max_table_grid_cells.to_string(),
        ),
        (
            "--max-unicode-cmap-mappings",
            defaults.max_unicode_cmap_mappings.to_string(),
        ),
    ] {
        for arguments in [
            vec!["--disable-safety-limits", flag, value.as_str()],
            vec![flag, value.as_str(), "--disable-safety-limits"],
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_page"))
                .arg(noncompliant_fixture())
                .args(arguments)
                .output()
                .expect("reject conflicting safety-limit arguments");
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            let stderr = String::from_utf8(output.stderr).expect("UTF-8 conflict");
            assert!(stderr.contains("cannot be used with"), "{stderr}");
            assert!(stderr.contains(flag), "{stderr}");
        }
    }
}

#[test]
fn disabling_safety_limits_preserves_validation_in_every_output_format() {
    for arguments in [
        vec![],
        vec!["--format", "details"],
        vec!["--format", "json"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_page"))
            .arg(noncompliant_fixture())
            .arg("--disable-safety-limits")
            .args(arguments)
            .output()
            .expect("validate with unlimited safety limits");
        assert_eq!(output.status.code(), Some(2));
        assert!(!output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn json_extension_infers_json_file_output() {
    let temporary = TempDirectory::new();
    let report_path = temporary.join("report.JSON");
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
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
    assert_eq!(report["compliant"], false);
}

#[test]
fn txt_extension_writes_plain_summary_and_replaces_existing_output() {
    let temporary = TempDirectory::new();
    let report_path = temporary.join("report.txt");
    fs::write(&report_path, "stale report").expect("seed existing report");
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg(noncompliant_fixture())
        .args(["--output", report_path.to_str().expect("UTF-8 report path")])
        .output()
        .expect("write text report");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    let contents = fs::read_to_string(report_path).expect("read text report");
    assert!(contents.starts_with("Result  : Non-conformant\nProfile : PDF/A-1b\n"));
    assert!(contents.contains("Time    : "));
    assert!(!contents.contains("✓ PDF syntax\n"));
    assert!(!contents.contains('\u{1b}'));
    assert_ne!(contents, "stale report");
}

#[test]
fn explicit_json_format_allows_an_extensionless_output() {
    let temporary = TempDirectory::new();
    let report_path = temporary.join("report");
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
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
        .arg(&fixture)
        .output()
        .expect("run PDF validation");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let summary = String::from_utf8(output.stdout).expect("UTF-8 summary");
    assert!(summary.starts_with("Result  : Non-conformant\nProfile : PDF/A-1b\nTime    : "));
    assert!(summary.ends_with("s\n"));
    assert!(!summary.contains('['));
}

#[test]
fn missing_declared_profile_is_an_explicit_error() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../page_validation/tests/fixtures/structural.pdf");
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg(&fixture)
        .output()
        .expect("run PDF validation without a declared profile");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).expect("UTF-8 error"),
        "error: document does not declare a PDF/A or PDF/UA validation profile; an explicit profile is required\n"
    );
}

#[test]
fn no_color_flag_preserves_plain_human_output() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../page_validation/tests/fixtures/structural.pdf");
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg(&fixture)
        .args(["--profile", "1b", "--format", "details", "--no-color"])
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
        .arg(&fixture)
        .args(["--profile", "1b", "--format", "details"])
        .output()
        .expect("run detailed PDF validation");
    let json = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg(&fixture)
        .args(["--profile", "1b", "--format", "json"])
        .output()
        .expect("run JSON PDF validation");

    assert_eq!(details.status.code(), Some(2));
    assert!(details.stderr.is_empty());
    let details = String::from_utf8(details.stdout).expect("UTF-8 details");
    assert!(details.starts_with("Result  : Non-conformant\nRules   : "));
    assert!(details.contains(" failed rule"));
    assert!(details.contains(" / "));
    assert!(details.contains(" failed check"));
    assert!(details.contains("\nProfile : PDF/A-1b\nTime    : "));
    assert!(details.contains("\n\n["));
    assert!(!details.contains("Checks: "));
    assert!(!details.contains("Document:"));
    assert!(details.contains("Time    :"));
    let detailed_failure_count = details.lines().filter(|line| line.starts_with('[')).count();
    let json: serde_json::Value = serde_json::from_slice(&json.stdout).expect("validation JSON");
    assert_eq!(
        detailed_failure_count,
        json["failures"].as_array().expect("JSON failures").len()
    );
    assert!(detailed_failure_count > 0);
}

#[test]
fn unimplemented_profiles_are_not_cli_choices() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../page_validation/tests/fixtures/structural.pdf");
    let profiles = ["4", "4e", "4f", "ua2"];

    for argument in profiles {
        let output = Command::new(env!("CARGO_BIN_EXE_page"))
            .arg(&fixture)
            .args(["--profile", argument])
            .output()
            .expect("reject an unimplemented profile");

        assert_eq!(output.status.code(), Some(2), "profile {argument}");
        assert!(output.stdout.is_empty(), "profile {argument}");
        let stderr = String::from_utf8(output.stderr).expect("UTF-8 error");
        assert!(
            stderr.contains(&format!("invalid value '{argument}'")),
            "{stderr}"
        );
        assert!(
            stderr.contains("possible values: 1b, 1a, 2b, 2a, 2u, 3b, 3a, 3u, ua1"),
            "{stderr}"
        );
    }
}

#[test]
fn validation_json_uses_the_stable_public_schema() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../page_validation/tests/fixtures/trailer-id-missing.pdf");
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg(&fixture)
        .args(["--format", "json"])
        .output()
        .expect("run PDF validation");

    assert_eq!(output.status.code(), Some(2));
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("validation JSON report");
    assert_eq!(report["file"], fixture.display().to_string());
    assert_eq!(report["profile"], "1b");
    assert_eq!(report["compliant"], false);
    assert!(report["error"].is_null());
    assert!(
        report["failures"]
            .as_array()
            .is_some_and(|failures| !failures.is_empty())
    );
    assert!(report["failures"][0]["rule"].is_string());
    assert!(report["failures"][0]["message"].is_string());
    assert!(report["rules"]["total"].is_number());
    assert!(report["rules"]["failed"].is_number());
    assert!(report["checks"]["failed"].is_number());
}

#[test]
fn validation_json_reports_parser_errors_separately() {
    let validation = Path::new(env!("CARGO_MANIFEST_DIR")).join("../page_validation");
    let malformed = validation.join("tests/fixtures/malformed.pdf");
    let parser = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg(malformed)
        .args(["--profile", "1b", "--format", "json"])
        .output()
        .expect("run malformed PDF validation");
    assert_eq!(parser.status.code(), Some(2));
    let parser: serde_json::Value = serde_json::from_slice(&parser.stdout).expect("parser JSON");
    assert_eq!(parser["compliant"], false);
    assert_eq!(parser["profile"], "1b");
    assert_eq!(parser["failures"], serde_json::json!([]));
    assert!(parser["rules"].is_null());
    assert!(parser["checks"].is_null());
    assert_eq!(parser["error"]["kind"], "parser");
}

#[test]
fn validation_json_reports_inferred_profile_errors_without_a_profile() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../page_validation/tests/fixtures/structural.pdf");
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg(fixture)
        .args(["--format", "json"])
        .output()
        .expect("run validation without an inferred profile");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("validation JSON report");
    assert!(report["profile"].is_null());
    assert_eq!(report["compliant"], false);
    assert_eq!(report["failures"], serde_json::json!([]));
    assert_eq!(report["error"]["kind"], "operational");
}

#[test]
fn missing_input_uses_the_json_error_schema_when_requested() {
    let missing =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../page_validation/tests/fixtures/missing.pdf");
    let without_json = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg(&missing)
        .args(["--profile", "1b"])
        .output()
        .expect("run missing PDF validation");
    let with_json = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg(&missing)
        .args(["--profile", "1b", "--format", "json"])
        .output()
        .expect("run missing PDF validation with JSON format");

    assert_eq!(with_json.status.code(), Some(1));
    assert!(with_json.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&with_json.stdout).expect("input error JSON");
    assert_eq!(report["file"], missing.display().to_string());
    assert_eq!(report["profile"], "1b");
    assert_eq!(report["compliant"], false);
    assert_eq!(report["error"]["kind"], "operational");
    assert_eq!(report["error"]["rule"], "INPUT-IO-001");

    let missing_file_error = std::io::Error::from_raw_os_error(2);
    assert_eq!(
        std::str::from_utf8(&without_json.stderr).expect("UTF-8 error"),
        format!(
            "error: could not read '{}': {missing_file_error}\n",
            missing.display(),
        )
    );
    assert_eq!(without_json.status.code(), Some(1));
    assert!(without_json.stdout.is_empty());
}
