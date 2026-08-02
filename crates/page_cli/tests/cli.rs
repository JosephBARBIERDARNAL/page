use std::path::Path;
use std::process::Command;

#[test]
fn page_help_exposes_the_flat_validation_interface() {
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg("--help")
        .output()
        .expect("run page --help");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 help");
    assert!(stdout.contains("Usage: page [OPTIONS] --profile <PROFILE> <FILE>"));
    assert!(stdout.contains("--format <FORMAT>"));
    assert!(stdout.contains("details, json"));
    assert!(!stdout.contains("--json"));
    assert!(
        stdout.contains(
            "a-1b, a-1a, a-2b, a-2a, a-2u, a-3b, a-3a, a-3u, a-4, a-4e, a-4f, ua-1, ua-2"
        )
    );
}

#[test]
fn default_validation_output_is_a_compact_summary() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../page_validation/tests/fixtures/structural.pdf");
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg(&fixture)
        .args(["--profile", "a-1b"])
        .output()
        .expect("run PDF validation");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let summary = String::from_utf8(output.stdout).expect("UTF-8 summary");
    assert_eq!(summary.lines().count(), 1);
    assert!(summary.starts_with("PDF/A-1b: "));
    assert!(summary.ends_with(" implemented checks failed\n"));
    assert!(!summary.contains('['));
}

#[test]
fn details_format_prints_every_failed_rule() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../page_validation/tests/fixtures/structural.pdf");
    let details = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg(&fixture)
        .args(["--profile", "a-1b", "--format", "details"])
        .output()
        .expect("run detailed PDF validation");
    let json = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg(&fixture)
        .args(["--profile", "a-1b", "--format", "json"])
        .output()
        .expect("run JSON PDF validation");

    assert_eq!(details.status.code(), Some(2));
    assert!(details.stderr.is_empty());
    let details = String::from_utf8(details.stdout).expect("UTF-8 details");
    let detailed_failure_count = details.lines().filter(|line| line.starts_with('[')).count();
    let json: serde_json::Value = serde_json::from_slice(&json.stdout).expect("validation JSON");
    assert_eq!(
        detailed_failure_count,
        json["failures"].as_array().expect("JSON failures").len()
    );
    assert!(detailed_failure_count > 0);
}

#[test]
fn future_profiles_are_recognized_and_reported_as_unimplemented() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../page_validation/tests/fixtures/structural.pdf");
    let profiles = [
        ("a-1a", "PDF/A-1a"),
        ("a-2a", "PDF/A-2a"),
        ("a-2b", "PDF/A-2b"),
        ("a-2u", "PDF/A-2u"),
        ("a-3a", "PDF/A-3a"),
        ("a-3b", "PDF/A-3b"),
        ("a-3u", "PDF/A-3u"),
        ("a-4", "PDF/A-4"),
        ("a-4e", "PDF/A-4e"),
        ("a-4f", "PDF/A-4f"),
        ("ua-1", "PDF/UA-1"),
        ("ua-2", "PDF/UA-2"),
    ];

    for (argument, display_name) in profiles {
        let output = Command::new(env!("CARGO_BIN_EXE_page"))
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
        .join("../page_validation/tests/fixtures/structural.pdf");
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg(&fixture)
        .args(["--profile", "a-1b", "--format", "json"])
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
        .arg(&missing)
        .args(["--profile", "a-1b"])
        .output()
        .expect("run missing PDF validation");
    let with_json = Command::new(env!("CARGO_BIN_EXE_page"))
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
