use std::collections::BTreeSet;

use pdf_validation::{SafetyLimits, ValidationProfile, validate_bytes};

#[allow(dead_code)]
mod common;

#[test]
fn atomic_metadata_cases_trigger_only_the_targeted_local_metadata_rule() {
    let cases = [
        (
            "missing_metadata",
            &[
                "PDFA1B-METADATA-STRUCTURE-001",
                "PDFA1B-ID-SCHEMA-001",
                "PDFA1B-ID-PART-001",
                "PDFA1B-ID-CONFORMANCE-001",
                "PDFA1B-INFO-TITLE-001",
                "PDFA1B-INFO-AUTHOR-001",
                "PDFA1B-INFO-SUBJECT-001",
                "PDFA1B-INFO-KEYWORDS-001",
                "PDFA1B-INFO-CREATOR-001",
                "PDFA1B-INFO-PRODUCER-001",
            ][..],
        ),
        ("missing_type", &["PDFA1B-METADATA-STRUCTURE-001"]),
        ("missing_subtype", &["PDFA1B-METADATA-STRUCTURE-001"]),
        ("metadata_filter", &["PDFA1B-METADATA-FILTER-001"]),
        (
            "malformed_xmp",
            &[
                "PDFA1B-XMP-001",
                "PDFA1B-ID-SCHEMA-001",
                "PDFA1B-ID-PART-001",
                "PDFA1B-ID-CONFORMANCE-001",
                "PDFA1B-INFO-TITLE-001",
                "PDFA1B-INFO-AUTHOR-001",
                "PDFA1B-INFO-SUBJECT-001",
                "PDFA1B-INFO-KEYWORDS-001",
                "PDFA1B-INFO-CREATOR-001",
                "PDFA1B-INFO-PRODUCER-001",
            ],
        ),
        (
            "missing_identification",
            &[
                "PDFA1B-ID-SCHEMA-001",
                "PDFA1B-ID-PART-001",
                "PDFA1B-ID-CONFORMANCE-001",
            ],
        ),
        ("wrong_part", &["PDFA1B-ID-PART-001"]),
        ("lowercase_conformance", &["PDFA1B-ID-CONFORMANCE-001"]),
        (
            "duplicate_identification",
            &["PDFA1B-ID-PART-001", "PDFA1B-ID-CONFORMANCE-001"],
        ),
        ("title_mismatch", &["PDFA1B-INFO-TITLE-001"]),
        ("author_mismatch", &["PDFA1B-INFO-AUTHOR-001"]),
        ("subject_mismatch", &["PDFA1B-INFO-SUBJECT-001"]),
        ("keywords_mismatch", &["PDFA1B-INFO-KEYWORDS-001"]),
        ("creator_mismatch", &["PDFA1B-INFO-CREATOR-001"]),
        ("producer_mismatch", &["PDFA1B-INFO-PRODUCER-001"]),
        ("creation_date_mismatch", &["PDFA1B-INFO-CREATIONDATE-001"]),
        ("creation_date_invalid", &["PDFA1B-INFO-CREATIONDATE-001"]),
        ("mod_date_mismatch", &["PDFA1B-INFO-MODDATE-001"]),
        ("author_multiple", &["PDFA1B-INFO-AUTHOR-001"]),
    ];
    let baseline = validate_bytes(
        &common::metadata_fixture("baseline_b"),
        ValidationProfile::PdfA1b,
        &SafetyLimits::default(),
    );
    let baseline_ids = baseline
        .failures
        .iter()
        .map(|failure| failure.rule_id)
        .collect::<BTreeSet<_>>();
    for (case, expected) in cases {
        let report = validate_bytes(
            &common::metadata_fixture(case),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        let case_ids = report
            .failures
            .iter()
            .map(|failure| failure.rule_id)
            .collect::<BTreeSet<_>>();
        let (added, _removed) = common::rule_delta(&baseline_ids, &case_ids);
        assert_eq!(
            added,
            expected.iter().copied().collect(),
            "{case}: unexpected failure delta: {:#?}",
            report.failures
        );
    }
}

#[test]
fn accepted_conformance_and_offset_equivalent_dates_pass_targeted_rules() {
    for case in [
        "baseline_b",
        "accepted_a",
        "creation_date_equivalent_offset",
    ] {
        let report = validate_bytes(
            &common::metadata_fixture(case),
            ValidationProfile::PdfA1b,
            &SafetyLimits::default(),
        );
        assert!(
            report.failures.iter().all(|failure| {
                !failure.rule_id.starts_with("PDFA1B-INFO-")
                    && failure.rule_id != "PDFA1B-ID-CONFORMANCE-001"
            }),
            "{case}: {:#?}",
            report.failures
        );
    }
}
