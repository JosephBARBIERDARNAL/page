use clap::ValueEnum;
use page_validation::{FailureCategory, ValidationReport};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum ReportFormat {
    #[default]
    Text,
    Json,
}

/// Serializes `value` as pretty JSON to stdout, or prints
/// `could not serialize {description}: ...` to stderr on failure.
/// Returns `0` on success and `1` on a serialization failure.
pub fn emit_json(value: &impl Serialize, description: &str) -> i32 {
    match serde_json::to_string_pretty(value) {
        Ok(json) => {
            println!("{json}");
            0
        }
        Err(error) => {
            eprintln!("could not serialize {description}: {error}");
            1
        }
    }
}

/// Stable JSON contract for `page --json`.
#[derive(Debug, Serialize)]
pub struct JsonValidationReport {
    pub file: String,
    pub profile: &'static str,
    pub valid: bool,
    pub failures: Vec<JsonFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonError>,
}

#[derive(Debug, Serialize)]
pub struct JsonFailure {
    pub rule: &'static str,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct JsonError {
    pub kind: &'static str,
    pub rule: &'static str,
    pub message: String,
}

impl JsonValidationReport {
    pub fn from_report(file: String, profile: &'static str, report: &ValidationReport) -> Self {
        let error = report
            .failures
            .iter()
            .find_map(|failure| match failure.category {
                FailureCategory::Parser => Some(JsonError {
                    kind: "parser",
                    rule: failure.rule_id,
                    message: failure.message.clone(),
                }),
                FailureCategory::Operational => Some(JsonError {
                    kind: "operational",
                    rule: failure.rule_id,
                    message: failure.message.clone(),
                }),
                FailureCategory::Metadata | FailureCategory::Conformance => None,
            });
        let failures = if error.is_some() {
            Vec::new()
        } else {
            report
                .failures
                .iter()
                .map(|failure| JsonFailure {
                    rule: failure.rule_id,
                    message: failure.message.clone(),
                })
                .collect()
        };
        Self {
            file,
            profile,
            valid: report.checks_passed,
            failures,
            error,
        }
    }
}
