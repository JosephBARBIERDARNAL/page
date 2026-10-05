//! Shared presentation helpers for the CLI binaries.

use page_validation::{ValidationError, ValidationErrorKind, ValidationReport};

/// Maps a validation report to the CLI status used for pass and fail results.
#[doc(hidden)]
pub fn validation_exit_code(report: &ValidationReport) -> i32 {
    if report.is_compliant { 0 } else { 2 }
}

/// Maps parser and conformance errors to `2`, and operational errors to `1`.
#[doc(hidden)]
pub fn validation_error_exit_code(error: &ValidationError) -> i32 {
    match error.kind() {
        ValidationErrorKind::Parser | ValidationErrorKind::Conformance => 2,
        _ => 1,
    }
}

#[doc(hidden)]
pub mod output;
#[doc(hidden)]
pub mod spinner;
