#[allow(dead_code)]
mod common;

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
