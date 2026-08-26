/// Configurable bounds that keep PDF parsing and inspection resource use predictable regardless of what an untrusted input contains.
///
/// Each field caps a distinct resource: the raw input size, a single decoded stream, the sum of all decoded content streams, the number of indirect objects, the depth of a chased reference chain, and the number of incremental-update revisions read from the cross-reference chain. Exceeding any of these bounds during validation produces a [`PdfError`](crate::PdfError) variant instead of letting parsing or inspection consume unbounded memory or CPU. [`Self::default`] uses this type's `DEFAULT_*` associated constants.
///
/// ## Examples
///
/// ```
/// use page_validation::SafetyLimits;
///
/// let limits = SafetyLimits {
///     max_input_size: 1024,
///     ..SafetyLimits::default()
/// };
/// assert_eq!(limits.max_input_size, 1024);
/// assert_eq!(limits.max_object_count, SafetyLimits::DEFAULT_MAX_OBJECT_COUNT);
/// ```
#[derive(Clone, Debug)]
pub struct SafetyLimits {
    pub max_input_size: u64,
    pub max_decoded_stream_size: usize,
    pub max_total_decoded_content_size: usize,
    pub max_object_count: usize,
    pub max_reference_depth: usize,
    pub max_xref_revisions: usize,
}

impl SafetyLimits {
    /// ISO 19005-1:2005, 6.1.12-7 permits at most this many indirect objects.
    pub const PDF_A1_MAX_INDIRECT_OBJECTS: usize = 8_388_607;
    pub const DEFAULT_MAX_INPUT_SIZE: u64 = 256 * 1024 * 1024;
    pub const DEFAULT_MAX_DECODED_STREAM_SIZE: usize = 32 * 1024 * 1024;
    pub const DEFAULT_MAX_TOTAL_DECODED_CONTENT_SIZE: usize = 256 * 1024 * 1024;
    pub const DEFAULT_MAX_OBJECT_COUNT: usize = 1_000_000;
    pub const DEFAULT_MAX_REFERENCE_DEPTH: usize = 256;
    pub const DEFAULT_MAX_XREF_REVISIONS: usize = 1_024;
}

impl Default for SafetyLimits {
    fn default() -> Self {
        Self {
            max_input_size: Self::DEFAULT_MAX_INPUT_SIZE,
            max_decoded_stream_size: Self::DEFAULT_MAX_DECODED_STREAM_SIZE,
            max_total_decoded_content_size: Self::DEFAULT_MAX_TOTAL_DECODED_CONTENT_SIZE,
            max_object_count: Self::DEFAULT_MAX_OBJECT_COUNT,
            max_reference_depth: Self::DEFAULT_MAX_REFERENCE_DEPTH,
            max_xref_revisions: Self::DEFAULT_MAX_XREF_REVISIONS,
        }
    }
}
