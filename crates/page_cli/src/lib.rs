//! Shared presentation helpers for the CLI binaries.

use page_validation::ValidationReport;

/// Maps a validation report to the CLI status used for pass and fail results.
pub fn validation_exit_code(report: &ValidationReport) -> i32 {
    if report.is_compliant { 0 } else { 2 }
}

#[doc(hidden)]
pub mod output;
#[doc(hidden)]
pub mod spinner;
