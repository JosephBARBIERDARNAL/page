//! A deliberately small foundation for PDF/A validation.
//!
//! The crate implements PDF/A-1, PDF/A-2, and PDF/A-3 validation rule sets. A successful report
//! is a validation result for the selected profile, not a guarantee that every possible PDF
//! producer defect is recoverable from malformed input.

mod actions;
mod annotations;
mod catalog;
mod content_support;
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub mod differential;
mod document_features;
mod error;
mod file_spec;
mod font_embedding;
mod font_encodings;
mod forms;
mod graphics;
mod icc_based;
mod json;
mod language;
mod limits;
mod metadata;
mod model;
mod object_limits;
mod object_resolution;
mod page_tree;
mod predefined_cmaps;
mod report;
mod stream_safety;
mod syntax;
mod unicode_names;
mod validation;
mod xobject;

pub use error::{PdfError, ValidationError};
pub use json::{JsonError, JsonErrorKind, JsonFailure, JsonValidationReport};
pub use limits::SafetyLimits;
pub use metadata::{DocumentMetadata, XmpMetadata};
pub use model::{
    FontSummary, IccHeader, OutputIntentSummary, OutputIntentsSummary, PdfDocument, PdfObjectId,
};
pub use report::{FailureCategory, ValidationCounts, ValidationFailure, ValidationReport};
pub use validation::{
    ComplianceResult, ValidationProfile, is_pdf_compliant, is_pdf_compliant_bytes, validate_pdf,
    validate_pdf_bytes,
};
