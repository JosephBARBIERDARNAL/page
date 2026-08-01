//! Maps onto specific "Current state" gaps named by this project's PDF/A-1B
//! font/CMap/encoding/embedding/glyph/metric `/goal`, each with a currently
//! passing, currently reachable piece of evidence — so the claim that a gap
//! is closed is something this suite proves on every run, not something
//! asserted in a chat summary that can go stale the moment the code moves.
//! Unlike `document_structure_goal_closure.rs`, this file does not yet cover
//! every gap the goal names — new tests are added here as each one closes.

use std::fs;

const PROFILE_PATH: &str = "tests/fixtures/PDFA-1B-1.28.xml";
const COVERAGE_PATH: &str = "tests/fixtures/pdfa-1b-coverage.json";

/// Acceptance criterion: "All font mappings currently marked `partial/proxy`
/// can be reclassified as exact or have a narrowly documented veraPDF-backed
/// exception." Reads the checked-in coverage inventory's `font` matrix
/// directly (not a chat summary) and asserts that every predicate still
/// marked `partial/proxy` has its precise remaining gap spelled out in the
/// inventory's mapping notes -- the "still `partial/proxy`: <reason>" phrase
/// consistently used for a documented exception, as opposed to a bare
/// `partial/proxy` tag with no stated reason.
#[test]
fn gap_every_partial_proxy_font_predicate_has_a_documented_exception() {
    let coverage: serde_json::Value =
        serde_json::from_slice(&fs::read(COVERAGE_PATH).expect("read coverage inventory"))
            .expect("parse coverage inventory");
    let predicates = coverage["font"]["predicates"]
        .as_array()
        .expect("font predicates array");
    assert!(!predicates.is_empty(), "font predicate matrix is empty");

    let mut undocumented = Vec::new();
    for predicate in predicates {
        let strength = predicate["implementation_strength"][0]
            .as_str()
            .expect("implementation_strength");
        if strength != "partial/proxy" {
            continue;
        }
        let rule_ids = predicate["implementation_path"]
            .as_array()
            .expect("implementation_path")
            .iter()
            .map(|value| value.as_str().expect("rule id string"))
            .collect::<Vec<_>>();
        let applicability = predicate["applicability"]
            .as_array()
            .expect("applicability array")
            .iter()
            .map(|value| value.as_str().expect("applicability string"))
            .collect::<Vec<_>>()
            .join(" ");
        let has_documented_exception = applicability
            .to_ascii_lowercase()
            .contains("still `partial/proxy`".to_ascii_lowercase().as_str());
        if !has_documented_exception {
            undocumented.push(rule_ids.join(", "));
        }
    }
    assert!(
        undocumented.is_empty(),
        "these partial/proxy font predicates have no documented exception in the coverage inventory \
         (add a \"still `partial/proxy`: <reason>\" sentence, or reclassify to exact): \
         {undocumented:?}"
    );
}

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
