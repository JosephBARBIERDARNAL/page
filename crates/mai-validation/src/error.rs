use thiserror::Error;

#[derive(Debug, Error)]
pub enum PdfError {
    #[error("could not read input: {0}")]
    Io(#[from] std::io::Error),

    #[error("input is {actual} bytes, exceeding the {limit}-byte limit")]
    InputTooLarge { actual: u64, limit: u64 },

    #[error("PDF parser rejected the input: {0}")]
    Parse(#[from] lopdf::Error),

    #[error("PDF contains {actual} objects, exceeding the {limit}-object limit")]
    TooManyObjects { actual: usize, limit: usize },

    #[error("reference chain exceeds the configured depth of {0}")]
    ReferenceDepth(usize),

    #[error("required object has an unexpected type: {0}")]
    UnexpectedObject(&'static str),

    #[error("XMP metadata stream exceeds the decoded-size limit: {0}")]
    XmpDecodeLimit(String),

    #[error("ICC output profile stream exceeds the decoded-size limit: {0}")]
    IccDecodeLimit(String),
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
                | Self::Parse(lopdf::Error::Decompress(
                    lopdf::DecompressError::MemoryLimitExceeded { .. }
                ))
        )
    }
}
