use std::{env, fs};

use page_validation::SafetyLimits;
use page_validation::differential::{DifferentialRunner, ReferenceConfig};

#[allow(dead_code)]
mod common;

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

/// The same fixture, confirmed directly against the real pinned veraPDF
/// binary when opted in (`VERAPDF_BIN` set).
#[test]
fn malformed_font_matches_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let runner = DifferentialRunner::new(ReferenceConfig::pinned(executable)).expect("veraPDF");
    let path = env::temp_dir().join(format!(
        "page-symbolic-cmap-malformed-maxp-{}.pdf",
        std::process::id()
    ));
    fs::write(&path, common::symbolic_cmap_with_malformed_maxp_fixture()).expect("write fixture");
    let report = runner.compare_file(&path, &SafetyLimits::default());
    let local_failures = common::failure_ids(&fs::read(&path).expect("read fixture"));
    assert!(
        local_failures.contains("PDFA1B-TRUETYPE-SYMBOLIC-CMAP-001"),
        "local should flag PDFA1B-TRUETYPE-SYMBOLIC-CMAP-001"
    );
    assert!(
        local_failures.contains("PDFA1B-TRUETYPE-GLYPH-PRESENCE-001"),
        "local should flag PDFA1B-TRUETYPE-GLYPH-PRESENCE-001"
    );
    assert!(
        !local_failures.contains("PDFA1B-FONT-EMBEDDING-001"),
        "local should not flag PDFA1B-FONT-EMBEDDING-001"
    );
    let reference = report.reference_result.expect("veraPDF result");
    let reference_failures = reference
        .failed_rule_ids
        .iter()
        .map(ToString::to_string)
        .collect::<std::collections::BTreeSet<_>>();
    assert!(
        reference_failures.contains("ISO 19005-1:2005:6.3.7:3"),
        "veraPDF should flag ISO 19005-1:2005:6.3.7:3"
    );
    assert!(
        reference_failures.contains("ISO 19005-1:2005:6.3.5:1"),
        "veraPDF should flag ISO 19005-1:2005:6.3.5:1"
    );
    assert!(
        !reference_failures.contains("ISO 19005-1:2005:6.3.4:1"),
        "veraPDF should not flag ISO 19005-1:2005:6.3.4:1"
    );
    fs::remove_file(path).expect("remove fixture");
}
