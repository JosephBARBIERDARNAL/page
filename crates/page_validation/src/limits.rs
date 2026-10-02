//! Defines the configurable resource ceilings used throughout PDF parsing and inspection.
//!
//! `SafetyLimits` supplies finite defaults, an unlimited preset, and chainable setters for input,
//! decoded streams, object traversal, Form execution, table grids, and Unicode CMap expansion.
//! A separate constant records the PDF/A-1 indirect-object conformance limit, which remains
//! distinct from configurable operational bounds.

/// Configurable bounds that keep PDF parsing and inspection resource use predictable regardless of what an untrusted input contains.
#[derive(Clone, Debug)]
#[non_exhaustive]
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
    /// Sets the maximum input size in bytes.
    #[must_use]
    pub const fn max_input_size(mut self, limit: u64) -> Self {
        self.max_input_size = limit;
        self
    }

    /// Sets the maximum decoded size of a single stream in bytes.
    #[must_use]
    pub const fn max_decoded_stream_size(mut self, limit: usize) -> Self {
        self.max_decoded_stream_size = limit;
        self
    }

    /// Sets the maximum combined decoded content and retained font-stream size in bytes.
    #[must_use]
    pub const fn max_total_decoded_content_size(mut self, limit: usize) -> Self {
        self.max_total_decoded_content_size = limit;
        self
    }

    /// Sets the maximum number of Form XObject invocations across a document.
    #[must_use]
    pub const fn max_form_invocations(mut self, limit: usize) -> Self {
        self.max_form_invocations = limit;
        self
    }

    /// Sets the maximum number of parsed indirect objects.
    #[must_use]
    pub const fn max_object_count(mut self, limit: usize) -> Self {
        self.max_object_count = limit;
        self
    }

    /// Sets the maximum depth of a reference chain.
    #[must_use]
    pub const fn max_reference_depth(mut self, limit: usize) -> Self {
        self.max_reference_depth = limit;
        self
    }

    /// Sets the maximum number of cross-reference revisions.
    #[must_use]
    pub const fn max_xref_revisions(mut self, limit: usize) -> Self {
        self.max_xref_revisions = limit;
        self
    }

    /// Sets the maximum row or column span of a table cell.
    #[must_use]
    pub const fn max_table_span(mut self, limit: usize) -> Self {
        self.max_table_span = limit;
        self
    }

    /// Sets the maximum number of rows in an inspected table grid.
    #[must_use]
    pub const fn max_table_grid_rows(mut self, limit: usize) -> Self {
        self.max_table_grid_rows = limit;
        self
    }

    /// Sets the maximum number of columns in an inspected table grid.
    #[must_use]
    pub const fn max_table_grid_columns(mut self, limit: usize) -> Self {
        self.max_table_grid_columns = limit;
        self
    }

    /// Sets the maximum number of cells in an inspected table grid.
    #[must_use]
    pub const fn max_table_grid_cells(mut self, limit: usize) -> Self {
        self.max_table_grid_cells = limit;
        self
    }

    /// Sets the maximum number of mappings expanded from one ToUnicode CMap.
    #[must_use]
    pub const fn max_unicode_cmap_mappings(mut self, limit: usize) -> Self {
        self.max_unicode_cmap_mappings = limit;
        self
    }

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
