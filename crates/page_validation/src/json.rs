//! Defines the stable serializable JSON view of a validation report for client applications.
//!
//! Report conversion copies the source, profile, compliance result, counts, and each rule failure's category and optional object location into shared output types. Parser and operational failures use a separate error field and omit conformance findings and counts, keeping the wire representation consistent across consumers.

use serde::Serialize;

use crate::error::ValidationErrorDisposition;
use crate::{
    FailureCategory, PageError, PdfObjectId, ValidationCheckCounts, ValidationCounts,
    ValidationProfile, ValidationReport,
};

/// Stable, serializable representation of a validation report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct JsonValidationReport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub profile: Option<ValidationProfile>,
    pub is_compliant: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<ValidationCounts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checks: Option<ValidationCheckCounts>,
    pub failures: Vec<JsonFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonError>,
}

/// A rule failure in the stable JSON report, with its category and optional indirect object location.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct JsonFailure {
    pub rule_id: String,
    pub message: String,
    pub object_id: Option<PdfObjectId>,
    pub category: FailureCategory,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct JsonError {
    pub kind: JsonErrorKind,
    pub rule_id: String,
    pub message: String,
}

/// A parser or operational JSON error category, with room for future categories.
///
/// ```compile_fail,E0004
/// use page_validation::JsonErrorKind;
/// fn label(kind: JsonErrorKind) -> &'static str {
///     match kind {
///         JsonErrorKind::Parser => "parser",
///         JsonErrorKind::Operational => "operational",
///     }
/// }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum JsonErrorKind {
    Parser,
    Operational,
}

impl JsonValidationReport {
    /// Converts a terminal validation error into the shared JSON representation.
    ///
    /// Parser and operational errors populate the `error` field.
    #[must_use]
    pub fn from_validation_error(
        source: Option<String>,
        profile: Option<ValidationProfile>,
        error: PageError,
    ) -> Self {
        let (rules, checks, failures, error) = match error.disposition() {
            ValidationErrorDisposition::Operational { rule_id } => (
                None,
                None,
                Vec::new(),
                Some(JsonError {
                    kind: JsonErrorKind::Operational,
                    rule_id: rule_id.to_owned(),
                    message: error.to_string(),
                }),
            ),
            ValidationErrorDisposition::Parser { rule_id } => (
                None,
                None,
                Vec::new(),
                Some(JsonError {
                    kind: JsonErrorKind::Parser,
                    rule_id: rule_id.to_owned(),
                    message: error.to_string(),
                }),
            ),
        };

        Self {
            source,
            profile,
            is_compliant: false,
            rules,
            checks,
            failures,
            error,
        }
    }
}

impl ValidationReport {
    /// Returns the stable, serializable JSON representation of this report.
    pub fn json_report(&self) -> JsonValidationReport {
        let failures = self
            .failures
            .iter()
            .map(|failure| JsonFailure {
                rule_id: failure.rule_id.clone(),
                message: failure.message.clone(),
                object_id: failure.object_id,
                category: failure.category,
            })
            .collect();

        JsonValidationReport {
            source: self
                .source
                .as_ref()
                .map(|source| source.display().to_string()),
            profile: Some(self.profile),
            is_compliant: self.is_compliant,
            rules: Some(self.rules),
            checks: Some(self.checks),
            failures,
            error: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{JsonErrorKind, JsonValidationReport};
    use crate::{
        FailureCategory, PageError, PdfError, PdfObjectId, ValidationCheckCounts, ValidationCounts,
        ValidationFailure, ValidationProfile, ValidationReport,
    };

    #[test]
    fn json_report_uses_the_stable_schema() {
        let report = ValidationReport {
            source: Some("document.pdf".into()),
            profile: ValidationProfile::PdfA1b,
            is_compliant: false,
            rules: ValidationCounts {
                total: 1,
                passed: 0,
                failed: 1,
            },
            checks: ValidationCheckCounts { failed: 1 },
            document: crate::model::PdfDocument::default(),
            failures: vec![ValidationFailure {
                rule_id: "RULE-001".to_owned(),
                message: "failed".to_owned(),
                object_id: Some(PdfObjectId {
                    object_number: 42,
                    generation: 3,
                }),
                category: FailureCategory::Metadata,
            }],
        };

        let json = report.json_report();
        let value = serde_json::to_value(json).expect("serialize JSON report");

        assert_eq!(value["source"], "document.pdf");
        assert_eq!(value["profile"], "1b");
        assert_eq!(value["is_compliant"], false);
        assert_eq!(value["rules"]["total"], 1);
        assert_eq!(value["rules"]["failed"], 1);
        assert_eq!(value["checks"]["failed"], 1);
        assert_eq!(value["failures"][0]["rule_id"], "RULE-001");
        assert_eq!(value["failures"][0]["category"], "metadata");
        assert_eq!(
            value["failures"][0]["object_id"],
            serde_json::json!({"object_number": 42, "generation": 3})
        );
        assert!(value.get("error").is_none());
        assert!(value.get("file").is_none());
        assert!(value.get("compliant").is_none());
        assert!(value["failures"][0].get("rule").is_none());
    }

    #[test]
    fn terminal_parser_errors_use_the_shared_json_error_path() {
        let json = JsonValidationReport::from_validation_error(
            None,
            Some(ValidationProfile::PdfA1b),
            PageError::Pdf(PdfError::UnexpectedObject("catalog")),
        );

        assert!(json.failures.is_empty());
        assert!(json.rules.is_none());
        assert!(json.checks.is_none());
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
            rules: ValidationCounts {
                total: 1,
                passed: 1,
                failed: 0,
            },
            checks: ValidationCheckCounts { failed: 0 },
            document: crate::model::PdfDocument::default(),
            failures: Vec::new(),
        };

        let value = serde_json::to_value(report.json_report()).expect("serialize JSON report");

        assert!(value.get("source").is_none());
    }
}
