//! Defines validation reports, categorized failures, and rule and failed-check counts.
//!
//! Inspectors supply raw findings that validation aggregates into report failures. Report
//! helpers convert terminal errors into categorized results, attach source paths, format
//! output, and derive exit codes while keeping operational failures distinct from PDF
//! conformance results.

use std::fmt;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{PdfError, ValidationError};
use crate::model::{PdfDocument, PdfObjectId};
use crate::validation::ValidationProfile;

/// The kind of problem a `ValidationFailure` represents, separating operational and parsing concerns from PDF/A or PDF/UA conformance itself.
///
/// `Operational` covers input that could not be read or exceeded a configured `SafetyLimits` bound; `Parser` covers input the strict PDF parser rejected outright; `Metadata` covers XMP or document-information problems; `Conformance` covers every other rule violation. `ValidationReport::has_operational_failure` and `ValidationReport::exit_code` both key off whether any recorded failure is `Operational`.
///
/// ## Examples
///
/// ```rs
/// use page_validation::FailureCategory;
///
/// assert!(FailureCategory::Operational < FailureCategory::Conformance);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCategory {
    Operational,
    Parser,
    Metadata,
    Conformance,
}

/// One recorded conformance, metadata, parser, or operational problem in a `ValidationReport`.
///
/// `rule_id` identifies the specific check (for example `PDFA1B-CATALOG-001`), `message` is a human-readable description, `object_id` is the indirect object the failure is attributed to when one applies, and `category` classifies the failure via `FailureCategory`. Multiple raw findings for the same rule are aggregated into as few `ValidationFailure` values as the rule allows before being placed in `ValidationReport::failures`.
///
/// ## Examples
///
/// ```rs
/// use page_validation::{FailureCategory, ValidationFailure};
///
/// let failure = ValidationFailure {
///     rule_id: "PDFA1B-CATALOG-001".to_owned(),
///     message: "document trailer does not resolve to a Catalog dictionary".to_owned(),
///     object_id: None,
///     category: FailureCategory::Conformance,
/// };
/// assert_eq!(failure.category, FailureCategory::Conformance);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ValidationFailure {
    pub rule_id: String,
    pub message: String,
    pub object_id: Option<PdfObjectId>,
    pub category: FailureCategory,
}

/// A single check's raw failure, recorded by an inspection module before
/// `validation.rs` aggregates same-rule failures into one [`ValidationFailure`].
#[derive(Clone, Debug)]
pub(crate) struct RuleFailure {
    pub(crate) object_id: Option<PdfObjectId>,
    pub(crate) description: String,
}

/// A tally of how many implemented rules ran against a document and how many of those passed or failed.
///
/// `total` is always `passed + failed`; it does not count rules that are not yet implemented for the report's `ValidationProfile`, so a `is_compliant` report can still be missing coverage that `ValidationProfile::implemented_check_count` and the corpus gate track separately.
///
/// ## Examples
///
/// ```rs
/// use page_validation::ValidationCounts;
///
/// let counts = ValidationCounts {
///     total: 5,
///     passed: 5,
///     failed: 0,
/// };
/// assert_eq!(counts.total, counts.passed + counts.failed);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ValidationCounts {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

/// A tally of failed checks. A check is one raw finding produced while evaluating a rule, so a
/// rule can contribute more than one failed check.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ValidationCheckCounts {
    pub failed: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ValidationReport {
    pub source: Option<PathBuf>,
    pub profile: ValidationProfile,
    pub is_compliant: bool,
    pub rules: ValidationCounts,
    pub checks: ValidationCheckCounts,
    pub document: Option<PdfDocument>,
    pub failures: Vec<ValidationFailure>,
}

impl ValidationReport {
    pub(crate) fn with_source(mut self, source: &Path) -> Self {
        self.source = Some(source.to_path_buf());
        self
    }

    pub(crate) fn parse_failure(profile: ValidationProfile, message: impl Into<String>) -> Self {
        Self::single_failure(profile, "PDF-PARSE-001", message, FailureCategory::Parser)
    }

    pub(crate) fn operational_failure(
        profile: ValidationProfile,
        rule_id: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::single_failure(profile, rule_id, message, FailureCategory::Operational)
    }

    pub(crate) fn conformance_failure(
        profile: ValidationProfile,
        rule_id: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::single_failure(profile, rule_id, message, FailureCategory::Conformance)
    }

    /// Converts a terminal validation error into a one-failure report with the matching category.
    ///
    /// Parser rejections become parser failures, configured resource limits become operational failures, and the PDF/A-1 indirect-object limit becomes a conformance failure.
    #[must_use]
    pub fn from_validation_error(profile: ValidationProfile, error: ValidationError) -> Self {
        match error {
            ValidationError::UnsupportedProfile(profile) => Self::operational_failure(
                profile,
                "PROFILE-001",
                format!("validation profile {profile} is not implemented yet"),
            ),
            ValidationError::InputIo(error) => {
                Self::operational_failure(profile, "INPUT-IO-001", error.to_string())
            }
            ValidationError::Pdf(PdfError::TooManyIndirectObjects { actual, limit }) => {
                let rule_id = match profile {
                    ValidationProfile::PdfA1a | ValidationProfile::PdfA1b => {
                        "PDFA1B-INDIRECT-OBJECT-COUNT-001"
                    }
                    ValidationProfile::PdfA2a => "PDFA2A-INDIRECT-OBJECT-COUNT-001",
                    ValidationProfile::PdfA2b => "PDFA2B-INDIRECT-OBJECT-COUNT-001",
                    ValidationProfile::PdfA2u => "PDFA2U-INDIRECT-OBJECT-COUNT-001",
                    ValidationProfile::PdfA3a => "PDFA3A-INDIRECT-OBJECT-COUNT-001",
                    ValidationProfile::PdfA3b => "PDFA3B-INDIRECT-OBJECT-COUNT-001",
                    ValidationProfile::PdfA3u => "PDFA3U-INDIRECT-OBJECT-COUNT-001",
                    ValidationProfile::PdfA4
                    | ValidationProfile::PdfA4e
                    | ValidationProfile::PdfA4f
                    | ValidationProfile::PdfUa1
                    | ValidationProfile::PdfUa2 => "PDF-INDIRECT-OBJECT-COUNT-001",
                };
                Self::conformance_failure(
                    profile,
                    rule_id,
                    format!(
                        "the document contains {actual} indirect objects, exceeding the indirect-object limit of {limit}"
                    ),
                )
            }
            ValidationError::Pdf(error) if error.is_safety_limit() => {
                Self::operational_failure(profile, "RESOURCE-LIMIT-001", error.to_string())
            }
            ValidationError::Pdf(error) => Self::parse_failure(profile, error.to_string()),
            error @ (ValidationError::MissingProfileDeclaration
            | ValidationError::InvalidProfileDeclaration(_)) => {
                Self::operational_failure(profile, "PROFILE-001", error.to_string())
            }
        }
    }

    fn single_failure(
        profile: ValidationProfile,
        rule_id: &'static str,
        message: impl Into<String>,
        category: FailureCategory,
    ) -> Self {
        let (rules, checks) = match category {
            FailureCategory::Metadata | FailureCategory::Conformance => (
                ValidationCounts {
                    total: 1,
                    passed: 0,
                    failed: 1,
                },
                ValidationCheckCounts { failed: 1 },
            ),
            FailureCategory::Operational | FailureCategory::Parser => (
                ValidationCounts::default(),
                ValidationCheckCounts::default(),
            ),
        };
        Self {
            source: None,
            profile,
            is_compliant: false,
            rules,
            checks,
            document: None,
            failures: vec![ValidationFailure {
                rule_id: rule_id.to_owned(),
                message: message.into(),
                object_id: None,
                category,
            }],
        }
    }

    /// Whether this report's failures include one recorded as operational
    /// (unreadable input, a configured safety limit, or report serialization)
    /// rather than a PDF/A conformance or parser finding.
    pub fn has_operational_failure(&self) -> bool {
        self.failures
            .iter()
            .any(|failure| failure.category == FailureCategory::Operational)
    }

    pub fn exit_code(&self) -> i32 {
        if self.has_operational_failure() {
            1
        } else if self.is_compliant {
            0
        } else {
            2
        }
    }
}

impl fmt::Display for ValidationReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut output = String::new();
        writeln!(output, "PDF/A validation")?;
        writeln!(output, "Profile: {}", self.profile)?;
        writeln!(
            output,
            "Result: {}",
            if self.is_compliant {
                "no failures in implemented checks"
            } else {
                "failed"
            }
        )?;
        writeln!(
            output,
            "Rules: {} passed, {} failed, {} total",
            self.rules.passed, self.rules.failed, self.rules.total
        )?;
        writeln!(output, "Checks: {} failed", self.checks.failed)?;
        if let Some(document) = &self.document {
            writeln!(
                output,
                "Document: PDF {}, {} page(s), {} object(s)",
                document.version, document.page_count, document.object_count
            )?;
        }
        for failure in &self.failures {
            write!(
                output,
                "[{}] {:?}: {}",
                failure.rule_id, failure.category, failure.message
            )?;
            if let Some(id) = failure.object_id {
                write!(output, " (object {} {})", id.object_number, id.generation)?;
            }
            writeln!(output)?;
        }
        formatter.write_str(&output)
    }
}

#[cfg(test)]
mod tests {
    use super::{FailureCategory, ValidationReport};
    use crate::{PdfError, ValidationError, ValidationProfile};

    #[test]
    fn parser_and_operational_failures_have_zero_validation_counts() {
        for category in [FailureCategory::Parser, FailureCategory::Operational] {
            let report = ValidationReport::single_failure(
                ValidationProfile::PdfA1b,
                "TEST-001",
                "failure",
                category,
            );

            assert_eq!(report.rules.total, 0);
            assert_eq!(report.rules.passed, 0);
            assert_eq!(report.rules.failed, 0);
            assert_eq!(report.checks.failed, 0);
        }
    }

    #[test]
    fn metadata_and_conformance_failures_have_one_failed_rule_and_check() {
        for category in [FailureCategory::Metadata, FailureCategory::Conformance] {
            let report = ValidationReport::single_failure(
                ValidationProfile::PdfA1b,
                "TEST-001",
                "failure",
                category,
            );

            assert_eq!(report.rules.total, 1);
            assert_eq!(report.rules.passed, 0);
            assert_eq!(report.rules.failed, 1);
            assert_eq!(report.checks.failed, 1);
        }
    }

    #[test]
    fn indirect_object_count_errors_use_the_active_profile_rule() {
        let cases = [
            (
                ValidationProfile::PdfA1b,
                "PDFA1B-INDIRECT-OBJECT-COUNT-001",
            ),
            (
                ValidationProfile::PdfA2b,
                "PDFA2B-INDIRECT-OBJECT-COUNT-001",
            ),
            (
                ValidationProfile::PdfA3u,
                "PDFA3U-INDIRECT-OBJECT-COUNT-001",
            ),
            (ValidationProfile::PdfUa1, "PDF-INDIRECT-OBJECT-COUNT-001"),
        ];

        for (profile, expected_rule_id) in cases {
            let report = ValidationReport::from_validation_error(
                profile,
                ValidationError::Pdf(PdfError::TooManyIndirectObjects {
                    actual: 8_388_608,
                    limit: 8_388_607,
                }),
            );
            assert_eq!(report.failures[0].rule_id, expected_rule_id);
            assert_eq!(report.rules.total, 1);
            assert_eq!(report.rules.failed, 1);
            assert_eq!(report.checks.failed, 1);
        }
    }
}
