use page_validation::SafetyLimits;

#[test]
fn setters_compose_on_the_unlimited_preset_in_const_contexts() {
    const LIMITS: SafetyLimits = SafetyLimits::unlimited()
        .with_max_input_size(1)
        .with_max_decoded_stream_size(2)
        .with_max_total_decoded_content_size(3)
        .with_max_form_invocations(4)
        .with_max_object_count(5)
        .with_max_reference_depth(6)
        .with_max_xref_revisions(7)
        .with_max_table_span(8)
        .with_max_table_grid_rows(9)
        .with_max_table_grid_columns(10)
        .with_max_table_grid_cells(11)
        .with_max_unicode_cmap_mappings(12);

    assert_eq!(LIMITS.max_input_size(), 1);
    assert_eq!(
        [
            LIMITS.max_decoded_stream_size(),
            LIMITS.max_total_decoded_content_size(),
            LIMITS.max_form_invocations(),
            LIMITS.max_object_count(),
            LIMITS.max_reference_depth(),
            LIMITS.max_xref_revisions(),
            LIMITS.max_table_span(),
            LIMITS.max_table_grid_rows(),
            LIMITS.max_table_grid_columns(),
            LIMITS.max_table_grid_cells(),
            LIMITS.max_unicode_cmap_mappings(),
        ],
        [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
    );
}

#[test]
fn setters_preserve_other_defaults_and_accept_zero() {
    let limits = SafetyLimits::default()
        .with_max_input_size(0)
        .with_max_reference_depth(0);

    assert_eq!(limits.max_input_size(), 0);
    assert_eq!(limits.max_reference_depth(), 0);
    assert_eq!(
        limits.max_object_count(),
        SafetyLimits::DEFAULT_MAX_OBJECT_COUNT
    );
    assert_eq!(
        limits.max_decoded_stream_size(),
        SafetyLimits::DEFAULT_MAX_DECODED_STREAM_SIZE,
    );
    assert_eq!(
        SafetyLimits::default().max_input_size(),
        SafetyLimits::DEFAULT_MAX_INPUT_SIZE,
    );
}
