use serde::Serialize;

use crate::{FailureCategory, ValidationProfile, ValidationReport};

/// Stable, serializable representation of a validation report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct JsonValidationReport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    pub profile: Option<ValidationProfile>,
    pub valid: bool,
    pub failures: Vec<JsonFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonError>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct JsonFailure {
    pub rule: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct JsonError {
    pub kind: JsonErrorKind,
    pub rule: String,
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
    pub fn json_report(&self) -> JsonValidationReport {
        let error = self
            .failures
            .iter()
            .find_map(|failure| match failure.category {
                FailureCategory::Parser => Some(JsonError {
                    kind: JsonErrorKind::Parser,
                    rule: failure.rule_id.clone(),
                    message: failure.message.clone(),
                }),
                FailureCategory::Operational => Some(JsonError {
                    kind: JsonErrorKind::Operational,
                    rule: failure.rule_id.clone(),
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
                    rule: failure.rule_id.clone(),
                    message: failure.message.clone(),
                })
                .collect()
        };

        JsonValidationReport {
            file: self
                .source
                .as_ref()
                .map(|source| source.display().to_string()),
            profile: Some(self.profile),
            valid: self.is_compliant,
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
            source: Some("document.pdf".into()),
            profile: ValidationProfile::PdfA1b,
            is_compliant: false,
            preliminary: true,
            checks: ValidationCounts {
                total: 1,
                passed: 0,
                failed: 1,
            },
            document: None,
            failures: vec![ValidationFailure {
                rule_id: "RULE-001".to_owned(),
                message: "failed".to_owned(),
                object_id: None,
                category: FailureCategory::Conformance,
            }],
        };

        let json = report.json_report();
        let value = serde_json::to_value(json).expect("serialize JSON report");

        assert_eq!(value["file"], "document.pdf");
        assert_eq!(value["profile"], "1b");
        assert_eq!(value["valid"], false);
        assert_eq!(value["failures"][0]["rule"], "RULE-001");
        assert!(value.get("error").is_none());
    }

    #[test]
    fn json_report_separates_parser_errors_from_failures() {
        let report = ValidationReport {
            source: None,
            profile: ValidationProfile::PdfA1b,
            is_compliant: false,
            preliminary: true,
            checks: ValidationCounts {
                total: 1,
                passed: 0,
                failed: 1,
            },
            document: None,
            failures: vec![ValidationFailure {
                rule_id: "PDF-PARSE-001".to_owned(),
                message: "malformed PDF".to_owned(),
                object_id: None,
                category: FailureCategory::Parser,
            }],
        };

        let json: JsonValidationReport = report.json_report();

        assert!(json.failures.is_empty());
        assert_eq!(
            json.error.expect("parser error").kind,
            JsonErrorKind::Parser
        );
    }

    #[test]
    fn json_report_omits_the_file_for_byte_input() {
        let report = ValidationReport {
            source: None,
            profile: ValidationProfile::PdfA1b,
            is_compliant: true,
            preliminary: true,
            checks: ValidationCounts {
                total: 1,
                passed: 1,
                failed: 0,
            },
            document: None,
            failures: Vec::new(),
        };

        let value = serde_json::to_value(report.json_report()).expect("serialize JSON report");

        assert!(value.get("file").is_none());
    }
}
