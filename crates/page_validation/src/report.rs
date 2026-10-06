//! Defines validation reports, categorized failures, their shared text formatting, and rule and failed-check counts.
//!
//! Inspectors supply raw findings that validation aggregates into report failures. Reports
//! contain metadata and conformance findings; parser, input, profile, and safety-limit errors
//! remain `PageError` values returned from validation entry points.

use std::fmt;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::model::{PdfDocument, PdfObjectId};
use crate::validation::ValidationProfile;

/// The kind of metadata or conformance problem a `ValidationFailure` represents.
///
/// Input, profile, parser, and safety-limit errors are returned as `PageError` and do not appear in a `ValidationReport`.
///
/// ## Examples
///
/// ```rs
/// use page_validation::FailureCategory;
///
/// assert!(FailureCategory::Metadata < FailureCategory::Conformance);
/// assert_eq!(FailureCategory::Metadata.to_string(), "Metadata");
/// ```
///
/// Matches outside this crate must handle future categories with a wildcard arm:
///
/// ```compile_fail,E0004
/// use page_validation::FailureCategory;
/// fn label(category: FailureCategory) -> &'static str {
///     match category {
///         FailureCategory::Metadata => "metadata",
///         FailureCategory::Conformance => "conformance",
///     }
/// }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FailureCategory {
    Metadata,
    Conformance,
}

impl fmt::Display for FailureCategory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Metadata => "Metadata",
            Self::Conformance => "Conformance",
        })
    }
}

/// One recorded metadata or conformance problem in a `ValidationReport`.
///
/// `rule_id` identifies the specific check (for example `PDFA1B-CATALOG-001`), `message` is a human-readable description, `object_id` is the indirect object the failure is attributed to when one applies, and `category` classifies the failure via `FailureCategory`. Multiple raw findings for the same rule are aggregated into as few `ValidationFailure` values as the rule allows before being placed in `ValidationReport::failures`.
///
/// Text formatting produces `[rule_id] Category: message`, followed by `(object number generation)` when an object is attributed to the failure.
///
/// ## Examples
///
/// ```rs
/// use page_validation::FailureCategory;
///
/// let category = FailureCategory::Conformance;
/// assert_eq!(category, FailureCategory::Conformance);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct ValidationFailure {
    pub rule_id: String,
    pub message: String,
    pub object_id: Option<PdfObjectId>,
    pub category: FailureCategory,
}

impl fmt::Display for ValidationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "[{}] {}: {}",
            self.rule_id, self.category, self.message
        )?;
        if let Some(id) = self.object_id {
            write!(
                formatter,
                " (object {} {})",
                id.object_number, id.generation
            )?;
        }
        Ok(())
    }
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
/// let mut counts = ValidationCounts::default();
/// counts.total = 5;
/// counts.passed = 5;
/// assert_eq!(counts.total, counts.passed + counts.failed);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct ValidationCounts {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

/// A tally of failed checks. A check is one raw finding produced while evaluating a rule, so a
/// rule can contribute more than one failed check.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct ValidationCheckCounts {
    pub failed: usize,
}

#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct ValidationReport {
    pub source: Option<PathBuf>,
    pub profile: ValidationProfile,
    pub is_compliant: bool,
    pub rules: ValidationCounts,
    pub checks: ValidationCheckCounts,
    /// The parsed document summary; terminal errors return no report.
    pub document: PdfDocument,
    pub failures: Vec<ValidationFailure>,
}

impl ValidationReport {
    pub(crate) fn with_source(mut self, source: &Path) -> Self {
        self.source = Some(source.to_path_buf());
        self
    }
}

impl fmt::Display for ValidationReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut output = String::new();
        let standard = match self.profile {
            ValidationProfile::PdfUa1 => "PDF/UA",
            _ => "PDF/A",
        };
        writeln!(output, "{standard} validation")?;
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
        writeln!(
            output,
            "Document: PDF {}, {} page(s), {} object(s)",
            self.document.version, self.document.page_count, self.document.object_count
        )?;
        for failure in &self.failures {
            writeln!(output, "{failure}")?;
        }
        formatter.write_str(&output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(profile: ValidationProfile) -> ValidationReport {
        ValidationReport {
            source: None,
            profile,
            is_compliant: true,
            rules: ValidationCounts::default(),
            checks: ValidationCheckCounts::default(),
            document: PdfDocument::default(),
            failures: Vec::new(),
        }
    }

    #[test]
    fn display_header_matches_validation_profile_family() {
        assert_eq!(
            report(ValidationProfile::PdfA1b).to_string().lines().next(),
            Some("PDF/A validation")
        );
        assert_eq!(
            report(ValidationProfile::PdfUa1).to_string().lines().next(),
            Some("PDF/UA validation")
        );
    }

    #[test]
    fn display_categories_use_human_readable_labels() {
        assert_eq!(FailureCategory::Metadata.to_string(), "Metadata");
        assert_eq!(FailureCategory::Conformance.to_string(), "Conformance");
    }

    #[test]
    fn display_failures_include_optional_object_attribution() {
        let mut failure = ValidationFailure {
            rule_id: "PDFA1B-ID-SCHEMA-001".to_owned(),
            message: "XMP does not contain the PDF/A Identification schema".to_owned(),
            object_id: None,
            category: FailureCategory::Metadata,
        };
        assert_eq!(
            failure.to_string(),
            "[PDFA1B-ID-SCHEMA-001] Metadata: XMP does not contain the PDF/A Identification schema"
        );

        failure.object_id = Some(PdfObjectId::from((53, 2)));
        assert_eq!(
            failure.to_string(),
            "[PDFA1B-ID-SCHEMA-001] Metadata: XMP does not contain the PDF/A Identification schema (object 53 2)"
        );
    }

    #[test]
    fn display_report_uses_shared_failure_formatting() {
        let mut report = report(ValidationProfile::PdfA1b);
        report.is_compliant = false;
        report.failures.push(ValidationFailure {
            rule_id: "PDFA1B-CATALOG-001".to_owned(),
            message: "Catalog is invalid".to_owned(),
            object_id: Some(PdfObjectId::from((7, 0))),
            category: FailureCategory::Conformance,
        });

        assert!(
            report
                .to_string()
                .ends_with("[PDFA1B-CATALOG-001] Conformance: Catalog is invalid (object 7 0)\n")
        );
    }
}
