use std::{env, fs};

use mai_validation::SafetyLimits;
use mai_validation::differential::{ComparisonClassification, DifferentialRunner, ReferenceConfig};

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
        ("packet_bytes_double", &["PDFA1B-XMP-PACKET-BYTES-001"]),
        ("packet_bytes_single", &["PDFA1B-XMP-PACKET-BYTES-001"]),
        ("packet_bytes_spaced", &["PDFA1B-XMP-PACKET-BYTES-001"]),
        ("packet_encoding", &["PDFA1B-XMP-PACKET-ENCODING-001"]),
        (
            "packet_bytes_and_encoding",
            &[
                "PDFA1B-XMP-PACKET-BYTES-001",
                "PDFA1B-XMP-PACKET-ENCODING-001",
            ],
        ),
        ("packet_uppercase_bytes", &[]),
        ("packet_unquoted_bytes", &[]),
        ("packet_substring_bytes", &["PDFA1B-XMP-PACKET-BYTES-001"]),
        ("packet_body_bytes", &[]),
        ("packet_end_bytes", &[]),
        ("packet_first_forbidden_then_clean", &[]),
        (
            "packet_clean_then_forbidden",
            &["PDFA1B-XMP-PACKET-BYTES-001"],
        ),
        ("id_alias_declaration_only", &[]),
        ("id_part_alias", &["PDFA1B-ID-PART-PREFIX-001"]),
        (
            "id_conformance_alias",
            &["PDFA1B-ID-CONFORMANCE-PREFIX-001"],
        ),
        ("id_amd_canonical", &[]),
        ("id_amd_alias", &["PDFA1B-ID-AMD-PREFIX-001"]),
        ("id_part_default_element", &[]),
        ("id_conformance_default_element", &[]),
        ("id_amd_default_element", &[]),
        ("extension_valid", &[]),
        ("extension_rational_value_type", &[]),
        (
            "gps_coordinate_invalid",
            &["PDFA1B-XMP-PREDEFINED-VALUE-TYPE-001"],
        ),
        (
            "extension_undefined_field",
            &["PDFA1B-XMP-EXTENSION-FIELDS-001"],
        ),
        (
            "extension_container_prefix",
            &["PDFA1B-XMP-EXTENSION-CONTAINER-001"],
        ),
        (
            "extension_container_seq",
            &["PDFA1B-XMP-EXTENSION-CONTAINER-001"],
        ),
        (
            "extension_schema_name_prefix",
            &["PDFA1B-XMP-EXTENSION-SCHEMA-NAME-001"],
        ),
        (
            "extension_schema_namespace_prefix",
            &["PDFA1B-XMP-EXTENSION-SCHEMA-NAMESPACE-001"],
        ),
        (
            "extension_schema_prefix_prefix",
            &["PDFA1B-XMP-EXTENSION-SCHEMA-PREFIX-001"],
        ),
        (
            "extension_property_bag",
            &["PDFA1B-XMP-EXTENSION-SCHEMA-PROPERTIES-001"],
        ),
        (
            "extension_value_type_bag",
            &["PDFA1B-XMP-EXTENSION-SCHEMA-VALUE-TYPES-001"],
        ),
        (
            "extension_property_name_prefix",
            &["PDFA1B-XMP-EXTENSION-PROPERTY-NAME-001"],
        ),
        (
            "extension_property_value_type_prefix",
            &["PDFA1B-XMP-EXTENSION-PROPERTY-VALUE-TYPE-001"],
        ),
        (
            "extension_property_unknown_value_type",
            &["PDFA1B-XMP-EXTENSION-PROPERTY-VALUE-TYPE-001"],
        ),
        (
            "extension_property_category_prefix",
            &["PDFA1B-XMP-EXTENSION-PROPERTY-CATEGORY-001"],
        ),
        (
            "extension_property_bad_category",
            &["PDFA1B-XMP-EXTENSION-PROPERTY-CATEGORY-001"],
        ),
        (
            "extension_property_description_prefix",
            &["PDFA1B-XMP-EXTENSION-PROPERTY-DESCRIPTION-001"],
        ),
        (
            "extension_value_type_name_prefix",
            &["PDFA1B-XMP-EXTENSION-VALUE-TYPE-NAME-001"],
        ),
        (
            "extension_value_type_namespace_prefix",
            &["PDFA1B-XMP-EXTENSION-VALUE-TYPE-NAMESPACE-001"],
        ),
        (
            "extension_value_type_prefix_prefix",
            &["PDFA1B-XMP-EXTENSION-VALUE-TYPE-PREFIX-001"],
        ),
        (
            "extension_value_type_description_prefix",
            &["PDFA1B-XMP-EXTENSION-VALUE-TYPE-DESCRIPTION-001"],
        ),
        (
            "extension_field_bag",
            &["PDFA1B-XMP-EXTENSION-VALUE-TYPE-FIELDS-001"],
        ),
        (
            "extension_field_name_prefix",
            &["PDFA1B-XMP-EXTENSION-FIELD-NAME-001"],
        ),
        (
            "extension_field_value_type_prefix",
            &["PDFA1B-XMP-EXTENSION-FIELD-VALUE-TYPE-001"],
        ),
        (
            "extension_field_unknown_value_type",
            &["PDFA1B-XMP-EXTENSION-FIELD-VALUE-TYPE-001"],
        ),
        (
            "extension_field_description_prefix",
            &["PDFA1B-XMP-EXTENSION-FIELD-DESCRIPTION-001"],
        ),
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
        (
            "creation_date_invalid",
            &[
                "PDFA1B-INFO-CREATIONDATE-001",
                "PDFA1B-XMP-PREDEFINED-VALUE-TYPE-001",
            ],
        ),
        ("mod_date_mismatch", &["PDFA1B-INFO-MODDATE-001"]),
        ("author_multiple", &["PDFA1B-INFO-AUTHOR-001"]),
    ];
    let baseline_ids = common::failure_ids(&common::metadata_fixture("baseline_b"));
    for (case, expected) in cases {
        let bytes = common::metadata_fixture(case);
        let report = common::validate(&bytes);
        let case_ids = common::failure_ids(&bytes);
        let (added, _removed) = common::rule_delta(&baseline_ids, &case_ids);
        assert_eq!(
            added,
            expected.iter().map(|rule| (*rule).to_owned()).collect(),
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
        let report = common::validate(&common::metadata_fixture(case));
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

#[test]
fn custom_extension_type_shape_failure_is_reported() {
    let failures = common::failure_ids(&common::metadata_fixture("extension_custom_value_invalid"));
    assert!(failures.contains("PDFA1B-XMP-EXTENSION-PROPERTY-VALUE-SHAPE-001"));
}

#[test]
fn normalized_extension_value_types_match_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let path = env::temp_dir().join(format!(
        "mai-extension-rational-value-type-{}.pdf",
        std::process::id()
    ));
    fs::write(
        &path,
        common::metadata_fixture("extension_rational_value_type"),
    )
    .expect("write extension fixture");
    let runner = DifferentialRunner::new(ReferenceConfig::pinned(executable)).expect("veraPDF");
    let report = runner.compare_file(&path, &SafetyLimits::default());
    assert_eq!(report.classification, ComparisonClassification::Agreement);
    assert!(
        report
            .reference_result
            .expect("veraPDF result")
            .failed_rule_ids
            .is_empty(),
        "the schema must not fail either pinned value-type predicate"
    );
    fs::remove_file(path).expect("remove extension fixture");
}

#[test]
fn invalid_gps_coordinate_matches_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let path = env::temp_dir().join(format!(
        "mai-invalid-gps-coordinate-{}.pdf",
        std::process::id()
    ));
    fs::write(&path, common::metadata_fixture("gps_coordinate_invalid"))
        .expect("write GPS fixture");
    let runner = DifferentialRunner::new(ReferenceConfig::pinned(executable)).expect("veraPDF");
    let report = runner.compare_file(&path, &SafetyLimits::default());
    assert!(
        common::failure_ids(&fs::read(&path).expect("read GPS fixture"))
            .contains("PDFA1B-XMP-PREDEFINED-VALUE-TYPE-001")
    );
    assert!(
        report
            .reference_result
            .expect("veraPDF result")
            .failed_rule_ids
            .iter()
            .map(ToString::to_string)
            .any(|rule| rule == "ISO 19005-1:2005:6.7.9:3")
    );
    fs::remove_file(path).expect("remove GPS fixture");
}
