use std::path::Path;
use std::process::Command;

#[test]
fn mai_help_exposes_the_validation_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_mai"))
        .arg("--help")
        .output()
        .expect("run mai --help");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 help");
    assert!(stdout.contains("Usage: mai <COMMAND>"));
    assert!(stdout.contains("validate"));
}

#[test]
fn validation_json_is_a_cli_owned_presentation_of_the_library_report() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../mai-validation/tests/fixtures/structural.pdf");
    let output = Command::new(env!("CARGO_BIN_EXE_mai"))
        .args(["validate", "--profile", "pdfa-1b", "--format", "json"])
        .arg(fixture)
        .output()
        .expect("run PDF validation");

    assert_eq!(output.status.code(), Some(2));
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("validation JSON report");
    assert_eq!(report["profile"], "pdfa-1b");
    assert_eq!(report["checks"]["total"], 108);
    assert_eq!(report["implemented_checks_passed"], false);
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
