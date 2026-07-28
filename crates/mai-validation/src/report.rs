use std::fmt;
use std::fmt::Write;

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ValidationCounts {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ValidationReport {
    pub profile: ValidationProfile,
    pub implemented_checks_passed: bool,
    pub preliminary: bool,
    pub disclaimer: &'static str,
    pub checks: ValidationCounts,
    pub document: Option<PdfDocument>,
    pub failures: Vec<ValidationFailure>,
}

impl ValidationReport {
    pub(crate) const DISCLAIMER: &'static str =
        "This report covers only preliminary checks and does not establish full PDF/A compliance.";

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

    fn single_failure(
        profile: ValidationProfile,
        rule_id: &'static str,
        message: impl Into<String>,
        category: FailureCategory,
    ) -> Self {
        Self {
            profile,
            implemented_checks_passed: false,
            preliminary: true,
            disclaimer: Self::DISCLAIMER,
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

    pub fn exit_code(&self) -> i32 {
        if self
            .failures
            .iter()
            .any(|failure| failure.category == FailureCategory::Operational)
        {
            1
        } else if self.implemented_checks_passed {
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
            if self.implemented_checks_passed {
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
        writeln!(output, "Note: {}", self.disclaimer)?;
        formatter.write_str(&output)
    }
}
