use std::collections::BTreeSet;

use mai_validation::{SafetyLimits, ValidationProfile, validate_bytes};

#[allow(dead_code)]
mod common;

const INTEGER: &str = "PDFA1B-INTEGER-RANGE-001";
const REAL: &str = "PDFA1B-REAL-RANGE-001";
const STRING: &str = "PDFA1B-STRING-LENGTH-001";
const NAME: &str = "PDFA1B-NAME-LENGTH-001";
const ARRAY: &str = "PDFA1B-ARRAY-LENGTH-001";
const DICTIONARY: &str = "PDFA1B-DICTIONARY-LENGTH-001";

#[test]
fn object_limit_cases_have_the_complete_expected_failure_delta() {
    let cases = [
        ("baseline", &[][..]),
        ("object_limits_at_boundary", &[]),
        ("object_integer_high", &[INTEGER]),
        ("object_integer_low", &[INTEGER]),
        ("object_real_high", &[REAL]),
        ("object_real_low", &[REAL]),
        ("object_string_long", &[STRING]),
        ("object_name_long", &[NAME]),
        ("object_dictionary_key_long", &[NAME]),
        ("object_array_long", &[ARRAY]),
        ("object_dictionary_long", &[DICTIONARY]),
    ];
    let baseline = failure_ids(&common::object_limit_fixture("baseline"));
    for (case, expected) in cases {
        let actual = failure_ids(&common::object_limit_fixture(case));
        let (added, removed) = common::rule_delta(&baseline, &actual);
        assert_eq!(
            added,
            expected.iter().map(|rule| (*rule).to_owned()).collect(),
            "{case}: unexpected added failures"
        );
        assert!(
            removed.is_empty(),
            "{case}: removed baseline failures {removed:?}"
        );
    }
}

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
        let report = validate(&common::object_limit_fixture(case));
        let failure = report
            .failures
            .iter()
            .find(|failure| failure.rule_id == rule_id)
            .expect("targeted object-limit failure");
        assert!(failure.object_id.is_some(), "{case}: missing object ID");
        assert_eq!(report.checks.total, 108);
        assert_eq!(report.checks.failed, 1);
        assert_eq!(report.checks.passed, 107);
    }
}

fn validate(bytes: &[u8]) -> mai_validation::ValidationReport {
    validate_bytes(bytes, ValidationProfile::PdfA1b, &SafetyLimits::default())
}

fn failure_ids(bytes: &[u8]) -> BTreeSet<String> {
    validate(bytes)
        .failures
        .into_iter()
        .map(|failure| failure.rule_id.to_owned())
        .collect()
}
