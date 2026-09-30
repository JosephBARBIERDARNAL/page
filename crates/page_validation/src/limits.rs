/// Configurable bounds that keep PDF parsing and inspection resource use predictable regardless of what an untrusted input contains.
///
/// Each field caps a distinct resource: the raw input size, a single decoded stream, the combined decoded content streams and font streams retained during inspection, the number of Form XObject invocations, the number of indirect objects, the depth of a chased reference chain, the number of incremental-update revisions read from the cross-reference chain, an individual table-cell span, the dimensions of an inspected table grid, or the number of mappings expanded from one ToUnicode CMap. Exceeding any of these bounds during validation produces a `PdfError` variant instead of letting parsing or inspection consume unbounded memory or CPU. `Self::default` uses this type's `DEFAULT_*` associated constants.
///
/// ## Examples
///
/// ```rs
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
    pub max_form_invocations: usize,
    pub max_object_count: usize,
    pub max_reference_depth: usize,
    pub max_xref_revisions: usize,
    pub max_table_span: usize,
    pub max_table_grid_rows: usize,
    pub max_table_grid_columns: usize,
    pub max_table_grid_cells: usize,
    pub max_unicode_cmap_mappings: usize,
}

impl SafetyLimits {
    /// Removes all configurable resource ceilings up to the platform's integer bounds.
    ///
    /// Use only with trusted files: validation may consume unrestricted memory and CPU.
    /// Cycle detection, arithmetic checks, parser checks, and PDF conformance limits remain active.
    /// Individual fields can be set to finite bounds after constructing this preset.
    ///
    /// ```rs
    /// use page_validation::SafetyLimits;
    ///
    /// let limits = SafetyLimits::unlimited();
    /// assert_eq!(limits.max_input_size, u64::MAX);
    /// ```
    pub const fn unlimited() -> Self {
        Self {
            max_input_size: u64::MAX,
            max_decoded_stream_size: usize::MAX,
            max_total_decoded_content_size: usize::MAX,
            max_form_invocations: usize::MAX,
            max_object_count: usize::MAX,
            max_reference_depth: usize::MAX,
            max_xref_revisions: usize::MAX,
            max_table_span: usize::MAX,
            max_table_grid_rows: usize::MAX,
            max_table_grid_columns: usize::MAX,
            max_table_grid_cells: usize::MAX,
            max_unicode_cmap_mappings: usize::MAX,
        }
    }

    /// ISO 19005-1:2005, 6.1.12-7 permits at most this many indirect objects.
    pub const PDF_A1_MAX_INDIRECT_OBJECTS: usize = 8_388_607;
    pub const DEFAULT_MAX_INPUT_SIZE: u64 = 256 * 1024 * 1024;
    pub const DEFAULT_MAX_DECODED_STREAM_SIZE: usize = 32 * 1024 * 1024;
    pub const DEFAULT_MAX_TOTAL_DECODED_CONTENT_SIZE: usize = 256 * 1024 * 1024;
    pub const DEFAULT_MAX_FORM_INVOCATIONS: usize = 10_000;
    pub const DEFAULT_MAX_OBJECT_COUNT: usize = 1_000_000;
    pub const DEFAULT_MAX_REFERENCE_DEPTH: usize = 256;
    pub const DEFAULT_MAX_XREF_REVISIONS: usize = 1_024;
    pub const DEFAULT_MAX_TABLE_SPAN: usize = 1_024;
    pub const DEFAULT_MAX_TABLE_GRID_ROWS: usize = 1_024;
    pub const DEFAULT_MAX_TABLE_GRID_COLUMNS: usize = 1_024;
    pub const DEFAULT_MAX_TABLE_GRID_CELLS: usize = 1_000_000;
    pub const DEFAULT_MAX_UNICODE_CMAP_MAPPINGS: usize = 1_000_000;
}

impl Default for SafetyLimits {
    fn default() -> Self {
        Self {
            max_input_size: Self::DEFAULT_MAX_INPUT_SIZE,
            max_decoded_stream_size: Self::DEFAULT_MAX_DECODED_STREAM_SIZE,
            max_total_decoded_content_size: Self::DEFAULT_MAX_TOTAL_DECODED_CONTENT_SIZE,
            max_form_invocations: Self::DEFAULT_MAX_FORM_INVOCATIONS,
            max_object_count: Self::DEFAULT_MAX_OBJECT_COUNT,
            max_reference_depth: Self::DEFAULT_MAX_REFERENCE_DEPTH,
            max_xref_revisions: Self::DEFAULT_MAX_XREF_REVISIONS,
            max_table_span: Self::DEFAULT_MAX_TABLE_SPAN,
            max_table_grid_rows: Self::DEFAULT_MAX_TABLE_GRID_ROWS,
            max_table_grid_columns: Self::DEFAULT_MAX_TABLE_GRID_COLUMNS,
            max_table_grid_cells: Self::DEFAULT_MAX_TABLE_GRID_CELLS,
            max_unicode_cmap_mappings: Self::DEFAULT_MAX_UNICODE_CMAP_MAPPINGS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SafetyLimits;

    #[test]
    fn unlimited_sets_every_resource_bound_to_its_native_maximum() {
        const LIMITS: SafetyLimits = SafetyLimits::unlimited();
        assert_eq!(LIMITS.max_input_size, u64::MAX);
        for limit in [
            LIMITS.max_decoded_stream_size,
            LIMITS.max_total_decoded_content_size,
            LIMITS.max_form_invocations,
            LIMITS.max_object_count,
            LIMITS.max_reference_depth,
            LIMITS.max_xref_revisions,
            LIMITS.max_table_span,
            LIMITS.max_table_grid_rows,
            LIMITS.max_table_grid_columns,
            LIMITS.max_table_grid_cells,
            LIMITS.max_unicode_cmap_mappings,
        ] {
            assert_eq!(limit, usize::MAX);
        }
    }
}
