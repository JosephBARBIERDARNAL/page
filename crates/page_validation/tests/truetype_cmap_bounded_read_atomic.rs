pub mod common;

/// Confirmed live against veraPDF 1.30.2: `PDFA1B-TRUETYPE-SYMBOLIC-CMAP-001`
/// reads a TrueType program's `cmap` table subtable count directly from the
/// SFNT table directory, independent of whether the rest of the font
/// (`maxp`, `hhea`, ...) otherwise parses. A font whose `cmap` table is
/// valid (2 subtables) but whose `maxp` table is truncated to 2 bytes still
/// fails this rule on veraPDF -- it must fail locally too, not be silently
/// skipped because the whole font doesn't parse as a `ttf_parser::Face`.
///
/// The same fixture also confirmed a second, independent fix: veraPDF still
/// considers this font's `/FontFile2` "embedded" (no
/// `PDFA1B-FONT-EMBEDDING-001`/`ISO 19005-1:2005:6.3.4:1` failure) despite
/// the malformed `/maxp` table, so `font_is_embedded`/`valid_sfnt` must not
/// gate on a full `ttf_parser::Face::parse` either -- both now use
/// `ttf_parser::RawFace`, which reads only the SFNT signature and table
/// directory.
#[test]
fn malformed_font_is_still_checked_and_still_counted_as_embedded() {
    let failures = common::failure_ids(&common::symbolic_cmap_with_malformed_maxp_fixture());
    assert!(failures.contains("PDFA1B-TRUETYPE-SYMBOLIC-CMAP-001"));
    assert!(
        failures.contains("PDFA1B-TRUETYPE-GLYPH-PRESENCE-001"),
        "the malformed maxp table must not suppress the independently readable glyph-presence check: {failures:?}"
    );
    assert!(
        !failures.contains("PDFA1B-FONT-EMBEDDING-001"),
        "a malformed /maxp table must not make the font count as unembedded: {failures:?}"
    );
}
