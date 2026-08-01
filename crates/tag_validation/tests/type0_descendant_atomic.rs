#[allow(dead_code)]
mod common;

/// Confirmed live against veraPDF 1.28.2: an indirect reference to the name
/// `/Identity` for `/CIDToGIDMap` resolves rendered CIDs to the same
/// glyphs a direct `/Identity` would -- `Scanner::resolve_cid_to_gid_map`
/// must resolve indirection before checking the name, not only before the
/// stream fallback (the same bug shape as `valid_cid_to_gid_map`, but this
/// one governs actual CID-to-GID glyph lookup, not just the rule check).
/// The fixture also mismatches `/DW` (999 vs. the real glyph 1 advance
/// width of 500) deliberately: an unresolvable map makes `glyph_for`
/// return `None`, which is a silent `continue`, not a pushed failure, so a
/// *matching*-width case can't distinguish "resolved and correct" from
/// "unresolved and silently skipped" -- only a genuine mismatch proves the
/// map was actually resolved and the width was actually checked.
#[test]
fn indirect_identity_cidtogidmap_resolves_the_same_glyphs_as_a_direct_one() {
    common::assert_case_deltas(
        common::type0_descendant_fixture,
        "baseline",
        &[(
            "indirect_identity_cidtogidmap",
            &["PDFA1B-TRUETYPE-GLYPH-WIDTH-001"],
        )],
    );
}

/// PDF32000 9.7.3 requires exactly one `/DescendantFonts` entry, and this
/// was confirmed live against veraPDF 1.28.2: it creates a `PDCIDFont`
/// object, and evaluates every per-font predicate against it (including
/// embedding and `/CIDToGIDMap`), only for `DescendantFonts[0]`. A second
/// entry is invisible to veraPDF's object model entirely, however broken,
/// so a local check must not independently flag it either.
#[test]
fn only_the_first_descendant_font_is_checked() {
    common::assert_case_deltas(
        common::type0_descendant_fixture,
        "baseline",
        &[
            ("second_descendant_unembedded", &[]),
            ("second_descendant_missing_cidtogidmap", &[]),
        ],
    );
}
