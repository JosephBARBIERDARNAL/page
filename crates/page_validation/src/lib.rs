//! Core PDF accessibility and compliance validation for PDF/A-1, PDF/A-2, PDF/A-3, and PDF/UA-1.
//!
//! File and byte entry points parse bounded input, normalize document metadata and resources,
//! run the inspections required by the selected profile, and return a validation report or a
//! compliance result. This crate exposes shared models, limits, errors, and report types for
//! the CLI and language bindings; a successful report covers the implemented profile rules.

mod actions;
mod annotations;
mod catalog;
mod content_support;
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
mod xml_safety;
mod xobject;

pub use error::{PdfError, ValidationError};
pub use json::{JsonError, JsonErrorKind, JsonFailure, JsonValidationReport};
pub use limits::SafetyLimits;
pub use metadata::{DocumentMetadata, XmpMetadata};
pub use model::{
    FontSummary, IccHeader, OutputIntentSummary, OutputIntentsSummary, PdfDocument, PdfObjectId,
};
pub use report::{
    FailureCategory, ValidationCheckCounts, ValidationCounts, ValidationFailure, ValidationReport,
};
pub use validation::{
    ComplianceResult, ValidationProfile, is_pdf_compliant, is_pdf_compliant_bytes, validate_pdf,
    validate_pdf_bytes, validate_pdf_bytes_fast, validate_pdf_fast,
};
