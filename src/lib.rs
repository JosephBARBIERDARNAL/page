//! A deliberately small foundation for PDF/A validation.
//!
//! The crate currently implements only a preliminary PDF/A-1b rule set. A
//! successful report is not proof of full PDF/A conformance.

pub mod differential;
mod error;
mod limits;
mod metadata;
mod model;
mod report;
mod validation;

pub use error::PdfError;
pub use limits::SafetyLimits;
pub use metadata::{DocumentMetadata, XmpMetadata};
pub use model::{FontSummary, PdfDocument, PdfObjectId};
pub use report::{FailureCategory, ValidationCounts, ValidationFailure, ValidationReport};
pub use validation::{ValidationProfile, ValidationRule, validate_bytes, validate_file};
