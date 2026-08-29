pub mod common;

const INTEGER: &str = "PDFA1B-INTEGER-RANGE-001";
const REAL: &str = "PDFA1B-REAL-RANGE-001";
const STRING: &str = "PDFA1B-STRING-LENGTH-001";
const NAME: &str = "PDFA1B-NAME-LENGTH-001";
const ARRAY: &str = "PDFA1B-ARRAY-LENGTH-001";
const DICTIONARY: &str = "PDFA1B-DICTIONARY-LENGTH-001";

#[test]
fn object_limit_failures_attach_the_offending_indirect_object() {
    for (case, rule_id) in [
        ("object_integer_high", INTEGER),
        ("object_real_high", REAL),
        ("object_string_long", STRING),
        ("object_name_long", NAME),
        ("object_array_long", ARRAY),
        ("object_dictionary_long", DICTIONARY),
    ] {
        let report = common::validate(&common::object_limit_fixture(case));
        let failure = common::assert_single_failure(&report, rule_id);
        assert!(failure.object_id.is_some(), "{case}: missing object ID");
    }
}
