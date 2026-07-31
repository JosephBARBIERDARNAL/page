//! Maps onto specific "Current state" gaps named by this project's PDF/A-1B
//! font/CMap/encoding/embedding/glyph/metric `/goal`, each with a currently
//! passing, currently reachable piece of evidence — so the claim that a gap
//! is closed is something this suite proves on every run, not something
//! asserted in a chat summary that can go stale the moment the code moves.
//! Unlike `document_structure_goal_closure.rs`, this file does not yet cover
//! every gap the goal names — new tests are added here as each one closes.

use std::fs;

const PROFILE_PATH: &str = "tests/fixtures/PDFA-1B-1.28.xml";

/// Gap: "Unicode mappings ... required by the pinned PDF/A-1B profile/API."
/// Confirmed by scanning the pinned, SHA256-verified profile XML directly
/// (not memory, not a summary): no `Unicode` or `ToUnicode` term appears
/// anywhere in the 129 pinned PDF/A-1B predicates. There is nothing for
/// this milestone to implement here because the profile itself has no such
/// predicate for flavour `1b` — implementing Unicode-mapping validation
/// anyway would add a restriction veraPDF 1.28.2 does not itself enforce,
/// which this project's own rule ("veraPDF is the source of truth") forbids.
#[test]
fn gap_no_unicode_mapping_predicates_exist_in_the_pinned_1b_profile() {
    let profile = fs::read_to_string(PROFILE_PATH).expect("read pinned profile");
    for forbidden in ["Unicode", "ToUnicode"] {
        assert!(
            !profile.contains(forbidden),
            "pinned profile now names {forbidden}; this milestone's claim that no Unicode-mapping \
             predicate exists must be re-audited against the new occurrence(s)"
        );
    }
}
