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
//! [`SafetyLimits`] with chainable setters or field assignment. Public report fields remain
//! directly accessible. Destructuring structs requires `..`, and enum matches require a
//! wildcard arm for future variants.
//!
//! ```
//! use page_validation::{FailureCategory, PdfError, ValidationProfile, ValidationReport};
//!
//! let report = ValidationReport::from_validation_error(
//!     ValidationProfile::PdfA1b,
//!     PdfError::UnexpectedObject("catalog").into(),
//! );
//! let ValidationReport { profile, .. } = &report;
//! assert_eq!(*profile, ValidationProfile::PdfA1b);
//! for failure in &report.failures {
//!     match failure.category {
//!         FailureCategory::Parser => assert_eq!(failure.rule_id, "PDF-PARSE-001"),
//!         _ => {}
//!     }
//! }
//! ```
//!
//! External callers cannot construct a report with struct update syntax:
//!
//! ```compile_fail,E0639
//! use page_validation::{PdfError, ValidationProfile, ValidationReport};
//!
//! let report = ValidationReport::from_validation_error(
//!     ValidationProfile::PdfA1b,
//!     PdfError::UnexpectedObject("catalog").into(),
//! );
//! let report = ValidationReport { ..report };
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
#[doc(hidden)]
pub use validation::{ComplianceResult, validate_pdf_lazy};
pub use validation::{
    ValidationOptions, ValidationProfile, is_pdf_compliant, is_pdf_compliant_bytes, validate_pdf,
    validate_pdf_bytes,
};
