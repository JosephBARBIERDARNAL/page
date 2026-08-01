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
    assert!(stdout.contains("--json"));
    assert!(stdout.contains("a-1b"));
}

#[test]
fn validation_json_uses_the_stable_public_schema() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../page_validation/tests/fixtures/structural.pdf");
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg(&fixture)
        .args(["--profile", "a-1b", "--json"])
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
fn validation_json_reports_parser_and_operational_errors_separately() {
    let validation = Path::new(env!("CARGO_MANIFEST_DIR")).join("../page_validation");
    let malformed = validation.join("tests/fixtures/malformed.pdf");
    let parser = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg(malformed)
        .args(["--profile", "a-1b", "--json"])
        .output()
        .expect("run malformed PDF validation");
    assert_eq!(parser.status.code(), Some(2));
    let parser: serde_json::Value = serde_json::from_slice(&parser.stdout).expect("parser JSON");
    assert_eq!(parser["valid"], false);
    assert_eq!(parser["failures"], serde_json::json!([]));
    assert_eq!(parser["error"]["kind"], "parser");

    let missing = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg(validation.join("tests/fixtures/missing.pdf"))
        .args(["--profile", "a-1b", "--json"])
        .output()
        .expect("run missing PDF validation");
    assert_eq!(missing.status.code(), Some(1));
    let missing: serde_json::Value = serde_json::from_slice(&missing.stdout).expect("missing JSON");
    assert_eq!(missing["valid"], false);
    assert_eq!(missing["failures"], serde_json::json!([]));
    assert_eq!(missing["error"]["kind"], "operational");
}

#[test]
fn missing_input_reports_a_direct_error_without_validation_output() {
    let missing =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../page_validation/tests/fixtures/missing.pdf");
    let output = Command::new(env!("CARGO_BIN_EXE_page"))
        .arg(&missing)
        .args(["--profile", "a-1b"])
        .output()
        .expect("run missing PDF validation");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 error");
    let missing_file_error = std::io::Error::from_raw_os_error(2);
    assert_eq!(
        stderr,
        format!(
            "error: could not read '{}': {missing_file_error}\n",
            missing.display(),
        )
    );
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
}
