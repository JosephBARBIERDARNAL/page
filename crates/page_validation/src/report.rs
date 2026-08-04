use std::fmt;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::model::{PdfDocument, PdfObjectId};
use crate::validation::ValidationProfile;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCategory {
    Operational,
    Parser,
    Metadata,
    Conformance,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ValidationFailure {
    pub rule_id: &'static str,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ValidationCounts {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

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
            preliminary: true,
            checks: ValidationCounts {
                total: 1,
                passed: 0,
                failed: 1,
            },
            document: None,
            failures: vec![ValidationFailure {
                rule_id,
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
