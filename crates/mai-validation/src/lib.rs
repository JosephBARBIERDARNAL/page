//! A deliberately small foundation for PDF/A validation.
//!
//! The crate currently implements only a preliminary PDF/A-1b rule set. A
//! successful report is not proof of full PDF/A conformance.

mod actions;
mod annotations;
pub mod differential;
mod document_features;
mod error;
mod font_embedding;
mod forms;
mod graphics;
mod icc_based;
mod limits;
mod metadata;
mod model;
mod report;
mod validation;
mod xobject;

pub use error::PdfError;
pub use limits::SafetyLimits;
pub use metadata::{DocumentMetadata, XmpMetadata};
pub use model::{
    FontSummary, IccHeader, OutputIntentSummary, OutputIntentsSummary, PdfDocument, PdfObjectId,
};
pub use report::{FailureCategory, ValidationCounts, ValidationFailure, ValidationReport};
pub use validation::{ValidationProfile, validate_bytes, validate_file};
