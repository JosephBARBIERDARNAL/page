//! Records a profile-level constraint on PDF/A-1B font validation: its
//! checked-in veraPDF profile contains no Unicode-mapping predicate.

use std::fs;

const PROFILE_PATH: &str = "tests/fixtures/PDFA-1B-1.28.xml";

#[test]
fn pdfa_1b_profile_has_no_unicode_mapping_predicates() {
    let profile = fs::read_to_string(PROFILE_PATH).expect("read checked-in profile");
    for forbidden in ["Unicode", "ToUnicode"] {
        assert!(
            !profile.contains(forbidden),
            "checked-in PDF/A-1B profile now names {forbidden}; re-audit font validation"
        );
    }
}
