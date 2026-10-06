//! Core PDF accessibility and compliance validation for PDF/A-1, PDF/A-2, PDF/A-3, and PDF/UA-1.
//!
//! File and byte entry points take shared `ValidationOptions`, parse bounded input, normalize
//! document metadata and resources, run the inspections required by the selected profile, and
//! return either an exhaustive validation report or a lazily evaluated compliance boolean. This
//! crate exposes shared models, limits, errors, and report types for the CLI and language
//! bindings; a successful report covers the implemented profile rules.
//!
//! # API evolution
//!
//! Public structs and enums are non-exhaustive so future releases can add fields and variants.
//! Construct values with their constructors, conversions, or `Default`; customize
//! [`SafetyLimits`] with chainable setters and inspect bounds through getters. Public report fields
//! remain directly accessible. Destructuring structs requires `..`, and enum matches require a
//! wildcard arm for future variants.
//!
//! ```
//! use page_validation::FailureCategory;
//!
//! let category = FailureCategory::Conformance;
//! assert_eq!(category, FailureCategory::Conformance);
//! ```
//!
//! Parser, input, profile, and safety-limit failures are returned as `PageError` values;
//! only metadata and conformance findings appear in `ValidationReport::failures`.
//!
//! [`PdfDocument`] exposes document summary fields. Metadata and inspection models
//! remain internal and cannot be imported by consumers:
//!
//! ```compile_fail,E0432
//! use page_validation::{
//!     DocumentMetadata, XmpMetadata, FontSummary, IccHeader,
//!     OutputIntentSummary, OutputIntentsSummary,
//! };
//! ```

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

pub use error::{PageError, PdfError, ValidationErrorKind};
pub use json::{JsonError, JsonErrorKind, JsonFailure, JsonValidationReport};
pub use limits::SafetyLimits;
pub use model::{PdfDocument, PdfObjectId};
pub use report::{
    FailureCategory, ValidationCheckCounts, ValidationCounts, ValidationFailure, ValidationReport,
};
#[doc(hidden)]
pub use validation::{ComplianceResult, validate_pdf_lazy};
pub use validation::{
    ParseValidationProfileError, ValidationOptions, ValidationProfile, is_pdf_compliant,
    is_pdf_compliant_bytes, validate_pdf, validate_pdf_bytes,
};
