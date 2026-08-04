use std::collections::BTreeSet;

mod common;

const PRESENCE: &str = "PDFA1B-TRUETYPE-GLYPH-PRESENCE-001";
const WIDTH: &str = "PDFA1B-TRUETYPE-GLYPH-WIDTH-001";

#[test]
fn embedded_truetype_program_must_supply_each_rendered_ascii_glyph() {
    let baseline = common::failure_ids(&common::font_fixture("baseline_embedded"));
    let actual = common::failure_ids(&common::font_fixture("tt_glyph_missing"));
    let (added, removed) = common::rule_delta(&baseline, &actual);
    assert_eq!(added, BTreeSet::from([PRESENCE.to_owned()]));
    assert!(removed.is_empty());
}

#[test]
fn embedded_truetype_program_width_must_agree_with_the_font_dictionary() {
    let baseline = common::failure_ids(&common::font_fixture("baseline_embedded"));
    assert!(!baseline.contains(WIDTH), "baseline failures: {baseline:?}");
    let actual = common::failure_ids(&common::font_fixture("tt_glyph_width_mismatch"));
    let (added, removed) = common::rule_delta(&baseline, &actual);
    assert_eq!(added, BTreeSet::from([WIDTH.to_owned()]));
    assert!(removed.is_empty());
}

#[test]
fn winansi_non_ascii_glyphs_use_the_standard_pdf_character_mapping() {
    let baseline = common::failure_ids(&common::font_fixture("tt_nonascii_winansi"));
    assert!(!baseline.contains(WIDTH), "baseline failures: {baseline:?}");
    let actual = common::failure_ids(&common::font_fixture("tt_nonascii_winansi_width_mismatch"));
    let (added, removed) = common::rule_delta(&baseline, &actual);
    assert_eq!(added, BTreeSet::from([WIDTH.to_owned()]));
    assert!(removed.is_empty());
}
