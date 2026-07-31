use std::collections::BTreeSet;

#[allow(dead_code)]
mod common;

const EMBEDDING: &str = "PDFA1B-FONT-EMBEDDING-001";

const CASES: &[(&str, bool)] = &[
    ("annotation_appearance_unembedded", true),
    ("widget_state_unembedded", true),
    ("pattern_unembedded", true),
    ("pattern_unused", false),
    ("type3_charproc_unembedded", true),
];

/// A font used only from an annotation appearance stream, a Pattern's own
/// content, or a Type3 glyph CharProc must still be checked for embedding,
/// exactly as veraPDF 1.28.2 checks it (confirmed live for each shape below
/// before this coverage was added).
#[test]
fn fonts_used_outside_page_and_form_content_are_still_checked_for_embedding() {
    let baseline = common::failure_ids(&common::font_content_source_fixture("baseline"));
    assert!(!baseline.contains(EMBEDDING));
    for (case, should_fail) in CASES {
        let actual = common::failure_ids(&common::font_content_source_fixture(case));
        let (added, removed) = common::rule_delta(&baseline, &actual);
        let expected = if *should_fail {
            BTreeSet::from([EMBEDDING.to_owned()])
        } else {
            BTreeSet::new()
        };
        assert_eq!(added, expected, "{case}: unexpected added failures");
        assert!(
            removed.is_empty(),
            "{case}: removed baseline failures {removed:?}"
        );
    }
}

/// A button Widget's non-selected appearance state (`/AS` names a different
/// state than the one using the unembedded font) is still walked -- veraPDF
/// does not restrict font discovery to the currently selected `/AS` state.
#[test]
fn widget_non_selected_appearance_state_is_still_scanned() {
    let failures = common::failure_ids(&common::font_content_source_fixture(
        "widget_state_unembedded",
    ));
    assert!(failures.contains(EMBEDDING));
}

/// A Pattern resource that is declared but never selected via `scn`/`SCN`
/// is not "used", so its font must not be flagged.
#[test]
fn undeclared_pattern_use_does_not_flag_its_font() {
    let failures = common::failure_ids(&common::font_content_source_fixture("pattern_unused"));
    assert!(!failures.contains(EMBEDDING));
}

/// veraPDF 1.28.2 walks a `/D` (down) appearance stream for font use just
/// like `/N`, even though `/D`'s mere presence already fails
/// `PDFA1B-ANNOTATION-AP-ENTRIES-001` on its own -- both failures are
/// independently real, not one masking the other.
#[test]
fn down_appearance_stream_is_also_scanned_for_font_use() {
    let failures = common::failure_ids(&common::font_content_source_fixture(
        "down_appearance_unembedded",
    ));
    assert!(failures.contains(EMBEDDING));
    assert!(failures.contains("PDFA1B-ANNOTATION-AP-ENTRIES-001"));
}
