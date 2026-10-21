pub mod common;

const INTEGER: &str = "ISO 19005-1:2005:6.1.12:1";
const REAL: &str = "ISO 19005-1:2005:6.1.12:2";
const STRING: &str = "ISO 19005-1:2005:6.1.12:3";
const NAME: &str = "ISO 19005-1:2005:6.1.12:4";
const ARRAY: &str = "ISO 19005-1:2005:6.1.12:5";
const DICTIONARY: &str = "ISO 19005-1:2005:6.1.12:6";

#[test]
fn object_limit_failures_are_reported_for_each_rule() {
    for (case, rule_id) in [
        ("object_integer_high", INTEGER),
        ("object_real_high", REAL),
        ("object_string_long", STRING),
        ("object_name_long", NAME),
        ("object_array_long", ARRAY),
        ("object_dictionary_long", DICTIONARY),
    ] {
        let report = common::validate(&common::object_limit_fixture(case));
        common::assert_single_failure(&report, rule_id);
    }
}
