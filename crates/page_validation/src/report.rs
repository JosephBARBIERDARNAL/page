use std::fmt;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::model::{PdfDocument, PdfObjectId};
use crate::validation::ValidationProfile;

/// The kind of problem a [`ValidationFailure`] represents, separating operational and parsing concerns from PDF/A or PDF/UA conformance itself.
///
/// `Operational` covers input that could not be read or exceeded a configured [`SafetyLimits`](crate::SafetyLimits) bound; `Parser` covers input the strict PDF parser rejected outright; `Metadata` covers XMP or document-information problems; `Conformance` covers every other rule violation. [`ValidationReport::has_operational_failure`] and [`ValidationReport::exit_code`] both key off whether any recorded failure is `Operational`.
///
/// ## Examples
///
/// ```
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

/// One recorded conformance, metadata, parser, or operational problem in a [`ValidationReport`].
///
/// `rule_id` identifies the specific check (for example `PDFA1B-CATALOG-001`), `message` is a human-readable description, `object_id` is the indirect object the failure is attributed to when one applies, and `category` classifies the failure via [`FailureCategory`]. Multiple raw findings for the same rule are aggregated into as few `ValidationFailure` values as the rule allows before being placed in [`ValidationReport::failures`].
///
/// ## Examples
///
/// ```
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

/// A tally of how many implemented checks ran against a document and how many of those passed or failed.
///
/// `total` is always `passed + failed`; it does not count checks for rules that are not yet implemented for the report's [`ValidationProfile`](crate::ValidationProfile), so a `checks_passed` report can still be missing coverage that [`ValidationProfile::implemented_check_count`](crate::ValidationProfile::implemented_check_count) and the corpus/differential tooling track separately.
///
/// ## Examples
///
/// ```
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

/// The outcome of validating one document against one [`ValidationProfile`](crate::ValidationProfile): whether it passed, how many checks ran, and every recorded [`ValidationFailure`].
///
/// `checks_passed` is `true` only when every implemented check for `profile` passed; `preliminary` marks the result as based on this crate's still-growing rule subset rather than full veraPDF conformance. `document` holds the normalized document used during validation, or `None` when validation stopped before one could be built. Use [`Self::exit_code`] to translate a report into the process exit status this crate's CLI relies on, and [`Self::has_operational_failure`] to check whether any recorded failure is [`FailureCategory::Operational`] rather than a conformance finding.
///
/// ## Examples
///
/// ```
/// use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};
///
/// let limits = SafetyLimits::default();
/// let report = validate_bytes_with_profile(b"not a pdf", ValidationProfile::PdfA1b, &limits);
/// assert_eq!(report.exit_code(), 2);
/// ```
#[derive(Clone, Debug, Serialize)]
pub struct ValidationReport {
    pub source: Option<PathBuf>,
    pub profile: ValidationProfile,
    pub checks_passed: bool,
    pub preliminary: bool,
    pub checks: ValidationCounts,
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

    fn single_failure(
        profile: ValidationProfile,
        rule_id: &'static str,
        message: impl Into<String>,
        category: FailureCategory,
    ) -> Self {
        Self {
            source: None,
            profile,
            checks_passed: false,
            preliminary: false,
            checks: ValidationCounts {
                total: 1,
                passed: 0,
                failed: 1,
            },
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
        } else if self.checks_passed {
            0
        } else {
            2
        }
    }
}

impl fmt::Display for ValidationReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut output = String::new();
        writeln!(output, "Preliminary PDF/A validation")?;
        writeln!(output, "Profile: {}", self.profile)?;
        writeln!(
            output,
            "Result: {}",
            if self.checks_passed {
                "no failures in implemented checks"
            } else {
                "failed"
            }
        )?;
        writeln!(
            output,
            "Checks: {} passed, {} failed, {} total",
            self.checks.passed, self.checks.failed, self.checks.total
        )?;
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
