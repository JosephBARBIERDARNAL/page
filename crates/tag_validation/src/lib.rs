//! A deliberately small foundation for PDF/A validation.
//!
//! The crate currently implements only a preliminary PDF/A-1b rule set. A
//! successful report is not proof of full PDF/A conformance.

mod actions;
mod annotations;
mod catalog;
mod content_support;
pub mod differential;
mod document_features;
mod error;
mod file_spec;
mod font_embedding;
mod font_encodings;
mod forms;
mod graphics;
mod icc_based;
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
