pub mod common;

/// Pins the known veraPDF robustness boundary for malformed TrueType encodings.
#[test]
fn malformed_encoding_type_does_not_panic_locally() {
    let _ = common::validate(&common::font_fixture("tt_symbolic_malformed_encoding"));
    let _ = common::validate(&common::font_fixture("tt_nonsymbolic_malformed_encoding"));
}
