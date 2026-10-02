//! Defines errors raised before a validation report can be completed, including PDF parsing,
//! resource limits, input I/O, and profile selection failures.
//!
//! `PdfError` describes parsing and inspection failures; `ValidationError` wraps these alongside
//! entry-point errors. Safety-limit classification lets reporting distinguish operational
//! failures from parser rejections and PDF conformance violations.

use std::error::Error as StdError;
use thiserror::Error;

use crate::validation::ValidationProfile;

/// Errors from parsing a PDF or inspecting its object graph before the validation rules run.
///
/// Each variant is either a strict-parser rejection (for example a malformed cross-reference table) or one of the configurable `SafetyLimits` bounds being exceeded, such as an oversized input or an over-deep reference chain. `ValidationError::Pdf` wraps this type for the public validation entry points, so callers that only need the top-level outcome can match on `ValidationError` instead.
///
/// ## Examples
///
/// ```rs
/// use page_validation::{SafetyLimits, ValidationError, ValidationOptions, validate_pdf_bytes};
///
/// let limits = SafetyLimits::default().max_input_size(4);
/// let options = ValidationOptions::default().limits(limits);
/// let error = validate_pdf_bytes(b"%PDF-1.4", &options).unwrap_err();
/// assert!(matches!(error, ValidationError::Pdf(_)));
/// ```
///
/// Matches outside this crate must include a wildcard arm for future parser and limit errors:
///
/// ```compile_fail,E0004
/// use page_validation::PdfError;
/// fn handle(error: PdfError) {
///     match error {
///         PdfError::InputTooLarge { .. } | PdfError::Parse(_)
///         | PdfError::TooManyObjects { .. } | PdfError::TooManyIndirectObjects { .. }
///         | PdfError::ReferenceDepth(_) | PdfError::UnexpectedObject(_)
///         | PdfError::XmpDecodeLimit(_) | PdfError::IccDecodeLimit(_)
///         | PdfError::ContentDecodeLimit(_) | PdfError::TotalContentDecodeLimit(_)
///         | PdfError::TotalDecodedStreamLimit(_) | PdfError::FormInvocationLimit(_)
///         | PdfError::TableSpanLimit { .. } | PdfError::TableGridLimit { .. }
///         | PdfError::UnicodeCmapMappingLimit { .. } | PdfError::FontDecodeLimit(_)
///         | PdfError::XfaDecodeLimit(_) => {}
///     }
/// }
/// ```
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PdfError {
    #[error("input is {actual} bytes, exceeding the {limit}-byte limit")]
    InputTooLarge { actual: u64, limit: u64 },

    #[error("PDF parser rejected the input: {0}")]
    Parse(#[source] Box<dyn StdError + Send + Sync>),

    #[error("PDF contains {actual} objects, exceeding the {limit}-object limit")]
    TooManyObjects { actual: usize, limit: usize },

    #[error("PDF contains {actual} indirect objects, exceeding the PDF/A-1 limit of {limit}")]
    TooManyIndirectObjects { actual: usize, limit: usize },

    #[error("reference chain exceeds the configured depth of {0}")]
    ReferenceDepth(usize),

    #[error("required object has an unexpected type: {0}")]
    UnexpectedObject(&'static str),

    #[error("XMP metadata stream exceeds the decoded-size limit: {0}")]
    XmpDecodeLimit(String),

    #[error("ICC profile stream exceeds the decoded-size limit: {0}")]
    IccDecodeLimit(String),

    #[error("a content stream exceeds the decoded-size limit of {0} bytes")]
    ContentDecodeLimit(usize),

    #[error("content streams exceed the total decoded-size limit of {0} bytes")]
    TotalContentDecodeLimit(usize),

    #[error(
        "decoded content and retained font streams exceed the total decoded-size limit of {0} bytes"
    )]
    TotalDecodedStreamLimit(usize),

    #[error("Form XObject invocations exceed the configured limit of {0}")]
    FormInvocationLimit(usize),

    #[error("table cell span {actual} exceeds the configured {limit} span limit")]
    TableSpanLimit { actual: usize, limit: usize },

    #[error(
        "table grid dimensions {rows}x{columns} violate configured limits: at most {max_rows} rows, {max_columns} columns, and {max_cells} cells"
    )]
    TableGridLimit {
        rows: usize,
        columns: usize,
        max_rows: usize,
        max_columns: usize,
        max_cells: usize,
    },

    #[error(
        "ToUnicode CMap expands to {actual} mappings, exceeding the configured {limit}-mapping limit"
    )]
    UnicodeCmapMappingLimit { actual: usize, limit: usize },

    #[error("embedded font program exceeds the decoded-size limit of {0} bytes")]
    FontDecodeLimit(usize),

    #[error("an XFA stream exceeds the decoded-size limit of {0} bytes")]
    XfaDecodeLimit(usize),
}

impl PdfError {
    pub(crate) fn parse(error: impl StdError + Send + Sync + 'static) -> Self {
        Self::Parse(Box::new(error))
    }

    pub(crate) fn is_safety_limit(&self) -> bool {
        match self {
            Self::InputTooLarge { .. }
            | Self::TooManyObjects { .. }
            | Self::ReferenceDepth(_)
            | Self::XmpDecodeLimit(_)
            | Self::IccDecodeLimit(_)
            | Self::ContentDecodeLimit(_)
            | Self::TotalContentDecodeLimit(_)
            | Self::TotalDecodedStreamLimit(_)
            | Self::FormInvocationLimit(_)
            | Self::TableSpanLimit { .. }
            | Self::TableGridLimit { .. }
            | Self::UnicodeCmapMappingLimit { .. }
            | Self::FontDecodeLimit(_)
            | Self::XfaDecodeLimit(_) => true,
            Self::Parse(error) => error.downcast_ref::<lopdf::Error>().is_some_and(|error| {
                matches!(
                    error,
                    lopdf::Error::Decompress(lopdf::DecompressError::MemoryLimitExceeded { .. })
                )
            }),
            _ => false,
        }
    }
}

/// The top-level error returned by the validation entry points when a document cannot be scored against a profile at all.
///
/// This is distinct from a `ValidationReport` recording failures: a report means the profile's rules ran and found conformance problems, while `ValidationError` means the input could not be read, parsed, or matched to a profile in the first place. `Self::Pdf` carries the lower-level `PdfError` from parsing or inspecting the object graph.
///
/// ## Examples
///
/// ```rs
/// use page_validation::{ValidationError, ValidationOptions, validate_pdf_bytes};
///
/// let error = validate_pdf_bytes(b"not a pdf", &ValidationOptions::default()).unwrap_err();
/// assert!(matches!(error, ValidationError::Pdf(_)));
/// ```
///
/// Matches outside this crate must include a wildcard arm for future validation errors:
///
/// ```compile_fail,E0004
/// use page_validation::ValidationError;
/// fn handle(error: ValidationError) {
///     match error {
///         ValidationError::InputIo(_) | ValidationError::Pdf(_)
///         | ValidationError::MissingProfileDeclaration
///         | ValidationError::InvalidProfileDeclaration(_)
///         | ValidationError::UnsupportedProfile(_) => {}
///     }
/// }
/// ```
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ValidationError {
    #[error("could not read input: {0}")]
    InputIo(#[from] std::io::Error),

    #[error("{0}")]
    Pdf(#[from] PdfError),

    #[error(
        "document does not declare a PDF/A or PDF/UA validation profile; an explicit profile is required"
    )]
    MissingProfileDeclaration,

    #[error("document has an invalid validation profile declaration: {0}")]
    InvalidProfileDeclaration(String),

    #[error("validation profile {0} is not implemented yet")]
    UnsupportedProfile(ValidationProfile),
}
