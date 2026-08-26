use thiserror::Error;

use crate::validation::ValidationProfile;

/// Errors from parsing a PDF or inspecting its object graph before the validation rules run.
///
/// Each variant is either a strict-parser rejection (for example a malformed cross-reference table) or one of the configurable `SafetyLimits` bounds being exceeded, such as an oversized input or an over-deep reference chain. `ValidationError::Pdf` wraps this type for the public `validate_bytes` and `validate_file` entry points, so callers that only need the top-level outcome can match on `ValidationError` instead.
///
/// ## Examples
///
/// ```rs
/// use page_validation::{SafetyLimits, ValidationError, validate_bytes};
///
/// let limits = SafetyLimits {
///     max_input_size: 4,
///     ..SafetyLimits::default()
/// };
/// let error = validate_bytes(b"%PDF-1.4", &limits).unwrap_err();
/// assert!(matches!(error, ValidationError::Pdf(_)));
/// ```
#[derive(Debug, Error)]
pub enum PdfError {
    #[error("input is {actual} bytes, exceeding the {limit}-byte limit")]
    InputTooLarge { actual: u64, limit: u64 },

    #[error("PDF parser rejected the input: {0}")]
    Parse(#[from] lopdf::Error),

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

    #[error("embedded font program exceeds the decoded-size limit of {0} bytes")]
    FontDecodeLimit(usize),

    #[error("an XFA stream exceeds the decoded-size limit of {0} bytes")]
    XfaDecodeLimit(usize),
}

impl PdfError {
    pub(crate) fn is_safety_limit(&self) -> bool {
        matches!(
            self,
            Self::InputTooLarge { .. }
                | Self::TooManyObjects { .. }
                | Self::ReferenceDepth(_)
                | Self::XmpDecodeLimit(_)
                | Self::IccDecodeLimit(_)
                | Self::ContentDecodeLimit(_)
                | Self::TotalContentDecodeLimit(_)
                | Self::FontDecodeLimit(_)
                | Self::XfaDecodeLimit(_)
                | Self::Parse(lopdf::Error::Decompress(
                    lopdf::DecompressError::MemoryLimitExceeded { .. }
                ))
        )
    }
}

/// The top-level error returned by `validate_bytes` and `validate_file` when a document cannot be scored against a profile at all.
///
/// This is distinct from a `ValidationReport` recording failures: a report means the profile's rules ran and found conformance problems, while `ValidationError` means the input could not be read, parsed, or matched to a profile in the first place. `Self::Pdf` carries the lower-level `PdfError` from parsing or inspecting the object graph.
///
/// ## Examples
///
/// ```rs
/// use page_validation::{SafetyLimits, ValidationError, validate_bytes};
///
/// let limits = SafetyLimits::default();
/// let error = validate_bytes(b"not a pdf", &limits).unwrap_err();
/// assert!(matches!(error, ValidationError::Pdf(_)));
/// ```
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("could not read input: {0}")]
    InputIo(#[from] std::io::Error),

    #[error("{0}")]
    Pdf(#[from] PdfError),

    #[error(
        "document does not declare a PDF/A or PDF/UA validation profile, declare it with --profile"
    )]
    MissingProfileDeclaration,

    #[error("document has an invalid validation profile declaration: {0}")]
    InvalidProfileDeclaration(String),

    #[error("validation profile {0} is not implemented yet")]
    UnsupportedProfile(ValidationProfile),
}
