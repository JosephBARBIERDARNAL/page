pub mod common;

/// The check reads a TrueType program's `cmap` table subtable count directly from the
/// SFNT table directory, independent of whether the rest of the font
/// (`maxp`, `hhea`, ...) otherwise parses. A font whose `cmap` table is
/// valid (2 subtables) but whose `maxp` table is truncated to 2 bytes still
/// fails the check locally rather than being silently skipped because the
/// whole font doesn't parse as a `ttf_parser::Face`.
///
/// The same fixture also confirms that the malformed `/maxp` table does not
/// make the font program unembedded, so `font_is_embedded`/`valid_sfnt` must not
/// gate on a full `ttf_parser::Face::parse` either -- both now use
/// `ttf_parser::RawFace`, which reads only the SFNT signature and table
/// directory.
#[test]
fn malformed_font_is_still_checked_and_still_counted_as_embedded() {
    let failures = common::failure_ids(&common::symbolic_cmap_with_malformed_maxp_fixture());
    assert!(failures.contains("ISO 19005-1:2005:6.3.7:3"));
    assert!(
        failures.contains("ISO 19005-1:2005:6.3.5:1"),
        "the malformed maxp table must not suppress the independently readable glyph-presence check: {failures:?}"
    );
    assert!(
        !failures.contains("ISO 19005-1:2005:6.3.4:1"),
        "a malformed /maxp table must not make the font count as unembedded: {failures:?}"
    );
}
