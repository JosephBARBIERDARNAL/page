use serde::Serialize;

use crate::{FailureCategory, ValidationProfile, ValidationReport};

/// Stable, serializable representation of a validation report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct JsonValidationReport {
    pub file: String,
    pub profile: ValidationProfile,
    pub valid: bool,
    pub failures: Vec<JsonFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonError>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct JsonFailure {
    pub rule: &'static str,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct JsonError {
    pub kind: JsonErrorKind,
    pub rule: &'static str,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JsonErrorKind {
    Parser,
    Operational,
}

impl ValidationReport {
    /// Returns the stable, serializable JSON representation of this report.
    pub fn json_report(&self, file: impl Into<String>) -> JsonValidationReport {
        let error = self
            .failures
            .iter()
            .find_map(|failure| match failure.category {
                FailureCategory::Parser => Some(JsonError {
                    kind: JsonErrorKind::Parser,
                    rule: failure.rule_id,
                    message: failure.message.clone(),
                }),
                FailureCategory::Operational => Some(JsonError {
                    kind: JsonErrorKind::Operational,
                    rule: failure.rule_id,
                    message: failure.message.clone(),
                }),
                FailureCategory::Metadata | FailureCategory::Conformance => None,
            });
        let failures = if error.is_some() {
            Vec::new()
        } else {
            self.failures
                .iter()
                .map(|failure| JsonFailure {
                    rule: failure.rule_id,
                    message: failure.message.clone(),
                })
                .collect()
        };

        JsonValidationReport {
            file: file.into(),
            profile: self.profile,
            valid: self.checks_passed,
            failures,
            error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{JsonErrorKind, JsonValidationReport};
    use crate::{
        FailureCategory, ValidationCounts, ValidationFailure, ValidationProfile, ValidationReport,
    };

    #[test]
    fn json_report_uses_the_stable_schema() {
        let report = ValidationReport {
            profile: ValidationProfile::PdfA1b,
            checks_passed: false,
            preliminary: true,
            checks: ValidationCounts {
                total: 1,
                passed: 0,
                failed: 1,
            },
            document: None,
            failures: vec![ValidationFailure {
                rule_id: "RULE-001",
                message: "failed".to_owned(),
                object_id: None,
                category: FailureCategory::Conformance,
            }],
        };

        let json = report.json_report("document.pdf");
        let value = serde_json::to_value(json).expect("serialize JSON report");

        assert_eq!(value["file"], "document.pdf");
        assert_eq!(value["profile"], "a-1b");
        assert_eq!(value["valid"], false);
        assert_eq!(value["failures"][0]["rule"], "RULE-001");
        assert!(value.get("error").is_none());
    }

    #[test]
    fn json_report_separates_parser_errors_from_failures() {
        let report = ValidationReport {
            profile: ValidationProfile::PdfA1b,
            checks_passed: false,
            preliminary: true,
            checks: ValidationCounts {
                total: 1,
                passed: 0,
                failed: 1,
            },
            document: None,
            failures: vec![ValidationFailure {
                rule_id: "PDF-PARSE-001",
                message: "malformed PDF".to_owned(),
                object_id: None,
                category: FailureCategory::Parser,
            }],
        };

        let json: JsonValidationReport = report.json_report("document.pdf");

        assert!(json.failures.is_empty());
        assert_eq!(
            json.error.expect("parser error").kind,
            JsonErrorKind::Parser
        );
    }
}
