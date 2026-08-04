use std::{collections::BTreeSet, env, fs};

use page_validation::SafetyLimits;
use page_validation::differential::{
    ComparisonClassification, DifferentialRunner, ReferenceConfig, ReferenceProfile,
};

pub mod common;

const CASES: &[(&str, &[&str])] = &[
    ("first_rdf_package", &[]),
    (
        "empty_xmpmeta_stops_search",
        &[
            "PDFA1B-ID-SCHEMA-001",
            "PDFA1B-INFO-TITLE-001",
            "PDFA1B-INFO-AUTHOR-001",
            "PDFA1B-INFO-SUBJECT-001",
            "PDFA1B-INFO-KEYWORDS-001",
            "PDFA1B-INFO-CREATOR-001",
            "PDFA1B-INFO-PRODUCER-001",
        ],
    ),
    ("ix_changes_ignored", &[]),
    (
        "rdf_cdata_literal",
        &[
            "PDFA1B-XMP-001",
            "PDFA1B-ID-SCHEMA-001",
            "PDFA1B-INFO-TITLE-001",
            "PDFA1B-INFO-AUTHOR-001",
            "PDFA1B-INFO-SUBJECT-001",
            "PDFA1B-INFO-KEYWORDS-001",
            "PDFA1B-INFO-CREATOR-001",
            "PDFA1B-INFO-PRODUCER-001",
        ],
    ),
    ("rdf_comment_between_properties", &[]),
    ("localized_language_normalization", &[]),
    (
        "rdf_nbsp_between_descriptions",
        &[
            "PDFA1B-XMP-001",
            "PDFA1B-ID-SCHEMA-001",
            "PDFA1B-INFO-TITLE-001",
            "PDFA1B-INFO-AUTHOR-001",
            "PDFA1B-INFO-SUBJECT-001",
            "PDFA1B-INFO-KEYWORDS-001",
            "PDFA1B-INFO-CREATOR-001",
            "PDFA1B-INFO-PRODUCER-001",
        ],
    ),
    ("deprecated_dc_namespace", &[]),
    ("lang_alt_partial_language", &[]),
    (
        "unknown_rdf_property",
        &[
            "PDFA1B-XMP-EXTENSION-PROPERTY-DEFINITION-001",
            "PDFA1B-XMP-EXTENSION-PROPERTY-VALUE-SHAPE-001",
        ],
    ),
    (
        "mismatched_rdf_about",
        &[
            "PDFA1B-XMP-001",
            "PDFA1B-ID-SCHEMA-001",
            "PDFA1B-INFO-TITLE-001",
            "PDFA1B-INFO-AUTHOR-001",
            "PDFA1B-INFO-SUBJECT-001",
            "PDFA1B-INFO-KEYWORDS-001",
            "PDFA1B-INFO-CREATOR-001",
            "PDFA1B-INFO-PRODUCER-001",
        ],
    ),
    (
        "rdf_resource_and_value",
        &[
            "PDFA1B-XMP-001",
            "PDFA1B-ID-SCHEMA-001",
            "PDFA1B-INFO-TITLE-001",
            "PDFA1B-INFO-AUTHOR-001",
            "PDFA1B-INFO-SUBJECT-001",
            "PDFA1B-INFO-KEYWORDS-001",
            "PDFA1B-INFO-CREATOR-001",
            "PDFA1B-INFO-PRODUCER-001",
        ],
    ),
    (
        "duplicate_producer",
        &[
            "PDFA1B-XMP-001",
            "PDFA1B-ID-SCHEMA-001",
            "PDFA1B-INFO-TITLE-001",
            "PDFA1B-INFO-AUTHOR-001",
            "PDFA1B-INFO-SUBJECT-001",
            "PDFA1B-INFO-KEYWORDS-001",
            "PDFA1B-INFO-CREATOR-001",
            "PDFA1B-INFO-PRODUCER-001",
        ],
    ),
    ("predefined_structured_field", &[]),
    ("predefined_structured_attribute_fields", &[]),
    ("info_empty_property_value", &[]),
    ("keywords_qualified_parse_type", &[]),
    ("title_qualified_expanded", &[]),
    ("keywords_default_element", &[]),
    ("reordered_top_level_properties", &[]),
    ("xmp_latin1_recovery", &[]),
    ("xmp_ascii_control_reference_recovery", &[]),
    ("xmp_del_reference_preserved", &["PDFA1B-INFO-PRODUCER-001"]),
    ("xmp_utf16le_without_bom", &[]),
    ("xmp_utf32be_without_bom", &[]),
    (
        "rdf_parse_type_literal",
        &[
            "PDFA1B-XMP-001",
            "PDFA1B-ID-SCHEMA-001",
            "PDFA1B-INFO-TITLE-001",
            "PDFA1B-INFO-AUTHOR-001",
            "PDFA1B-INFO-SUBJECT-001",
            "PDFA1B-INFO-KEYWORDS-001",
            "PDFA1B-INFO-CREATOR-001",
            "PDFA1B-INFO-PRODUCER-001",
        ],
    ),
    (
        "no_rdf_package",
        &[
            "PDFA1B-ID-SCHEMA-001",
            "PDFA1B-INFO-TITLE-001",
            "PDFA1B-INFO-AUTHOR-001",
            "PDFA1B-INFO-SUBJECT-001",
            "PDFA1B-INFO-KEYWORDS-001",
            "PDFA1B-INFO-CREATOR-001",
            "PDFA1B-INFO-PRODUCER-001",
        ],
    ),
    (
        "missing_metadata",
        &[
            "PDFA1B-METADATA-STRUCTURE-001",
            "PDFA1B-ID-SCHEMA-001",
            "PDFA1B-INFO-TITLE-001",
            "PDFA1B-INFO-AUTHOR-001",
            "PDFA1B-INFO-SUBJECT-001",
            "PDFA1B-INFO-KEYWORDS-001",
            "PDFA1B-INFO-CREATOR-001",
            "PDFA1B-INFO-PRODUCER-001",
        ],
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
    (
        "packet_body_bytes",
        &[
            "PDFA1B-XMP-001",
            "PDFA1B-ID-SCHEMA-001",
            "PDFA1B-INFO-TITLE-001",
            "PDFA1B-INFO-AUTHOR-001",
            "PDFA1B-INFO-SUBJECT-001",
            "PDFA1B-INFO-KEYWORDS-001",
            "PDFA1B-INFO-CREATOR-001",
            "PDFA1B-INFO-PRODUCER-001",
        ],
    ),
    ("packet_end_bytes", &[]),
    ("packet_first_forbidden_then_clean", &[]),
    (
        "packet_clean_then_forbidden",
        &["PDFA1B-XMP-PACKET-BYTES-001"],
    ),
    ("id_alias_declaration_only", &[]),
    ("id_part_alias", &["PDFA1B-ID-PART-PREFIX-001"]),
    ("id_part_plus_one", &[]),
    ("id_part_leading_zero", &[]),
    (
        "id_part_unicode_digit",
        &["PDFA1B-XMP-PREDEFINED-VALUE-TYPE-001"],
    ),
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
    (
        "extension_container_attribute",
        &["PDFA1B-XMP-EXTENSION-CONTAINER-001"],
    ),
    ("extension_custom_simple_type", &[]),
    (
        "extension_unknown_declared_property_used",
        &[
            "PDFA1B-XMP-EXTENSION-PROPERTY-DEFINITION-001",
            "PDFA1B-XMP-EXTENSION-PROPERTY-VALUE-SHAPE-001",
            "PDFA1B-XMP-EXTENSION-PROPERTY-VALUE-TYPE-001",
        ],
    ),
    (
        "extension_namespace_whitespace",
        &[
            "PDFA1B-XMP-EXTENSION-PROPERTY-DEFINITION-001",
            "PDFA1B-XMP-EXTENSION-PROPERTY-VALUE-SHAPE-001",
        ],
    ),
    (
        "extension_duplicate_namespace_replaces",
        &[
            "PDFA1B-XMP-EXTENSION-PROPERTY-DEFINITION-001",
            "PDFA1B-XMP-EXTENSION-PROPERTY-VALUE-SHAPE-001",
        ],
    ),
    ("extension_duplicate_property_keeps_first", &[]),
    ("extension_rational_value_type", &[]),
    (
        "extension_xpath_invalid",
        &["PDFA1B-XMP-EXTENSION-PROPERTY-VALUE-SHAPE-001"],
    ),
    (
        "gps_coordinate_invalid",
        &["PDFA1B-XMP-PREDEFINED-VALUE-TYPE-001"],
    ),
    (
        "predefined_unknown_property",
        &[
            "PDFA1B-XMP-PREDEFINED-PROPERTY-001",
            "PDFA1B-XMP-PREDEFINED-VALUE-TYPE-001",
        ],
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
            "PDFA1B-INFO-TITLE-001",
            "PDFA1B-INFO-AUTHOR-001",
            "PDFA1B-INFO-SUBJECT-001",
            "PDFA1B-INFO-KEYWORDS-001",
            "PDFA1B-INFO-CREATOR-001",
            "PDFA1B-INFO-PRODUCER-001",
        ],
    ),
    ("missing_identification", &["PDFA1B-ID-SCHEMA-001"]),
    ("missing_conformance", &["PDFA1B-ID-CONFORMANCE-001"]),
    ("wrong_part", &["PDFA1B-ID-PART-001"]),
    ("lowercase_conformance", &["PDFA1B-ID-CONFORMANCE-001"]),
    (
        "duplicate_identification",
        &[
            "PDFA1B-XMP-001",
            "PDFA1B-ID-SCHEMA-001",
            "PDFA1B-INFO-TITLE-001",
            "PDFA1B-INFO-AUTHOR-001",
            "PDFA1B-INFO-SUBJECT-001",
            "PDFA1B-INFO-KEYWORDS-001",
            "PDFA1B-INFO-CREATOR-001",
            "PDFA1B-INFO-PRODUCER-001",
        ],
    ),
    ("title_mismatch", &["PDFA1B-INFO-TITLE-001"]),
    ("title_whitespace_equivalent", &[]),
    ("title_pdfdoc_equivalent", &[]),
    ("title_utf16_equivalent", &[]),
    ("title_utf8_bom_equivalent", &[]),
    ("title_utf16_odd_equivalent", &[]),
    ("title_fallback_first_alternative", &[]),
    ("title_fallback_generic_x", &[]),
    ("author_mismatch", &["PDFA1B-INFO-AUTHOR-001"]),
    (
        "author_ordered_alt",
        &["PDFA1B-XMP-PREDEFINED-VALUE-TYPE-001"],
    ),
    ("subject_mismatch", &["PDFA1B-INFO-SUBJECT-001"]),
    ("keywords_mismatch", &["PDFA1B-INFO-KEYWORDS-001"]),
    ("keywords_xmp_whitespace", &["PDFA1B-INFO-KEYWORDS-001"]),
    ("creator_mismatch", &["PDFA1B-INFO-CREATOR-001"]),
    ("producer_mismatch", &["PDFA1B-INFO-PRODUCER-001"]),
    ("producer_trailing_nul", &[]),
    ("producer_two_trailing_nuls", &["PDFA1B-INFO-PRODUCER-001"]),
    ("creation_date_mismatch", &["PDFA1B-INFO-CREATIONDATE-001"]),
    ("creation_date_pdf_z00_equivalent", &[]),
    ("creation_date_pdf_z00_00_equivalent", &[]),
    ("creation_date_pdf_unicode_digits_equivalent", &[]),
    (
        "creation_date_historic_same_lexical",
        &["PDFA1B-INFO-CREATIONDATE-001"],
    ),
    ("creation_date_historic_shifted_equivalent", &[]),
    ("creation_date_long_fraction_equivalent", &[]),
    (
        "creation_date_invalid",
        &["PDFA1B-XMP-PREDEFINED-VALUE-TYPE-001"],
    ),
    (
        "creation_date_reduced_mismatch",
        &["PDFA1B-INFO-CREATIONDATE-001"],
    ),
    ("creation_date_submillisecond_equivalent", &[]),
    ("mod_date_mismatch", &["PDFA1B-INFO-MODDATE-001"]),
    ("author_multiple", &["PDFA1B-INFO-AUTHOR-001"]),
];

#[test]
fn metadata_cases_have_the_complete_expected_failure_delta() {
    common::assert_case_deltas(common::metadata_fixture, "baseline_b", CASES);
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
fn pdfa_1a_identification_cases_match_local_predicate() {
    for (case, expected) in [
        ("accepted_a", None),
        ("baseline_b", Some("PDFA1A-ID-CONFORMANCE-001")),
        ("missing_conformance", Some("PDFA1A-ID-CONFORMANCE-001")),
        ("lowercase_conformance", Some("PDFA1A-ID-CONFORMANCE-001")),
    ] {
        let report = page_validation::validate_bytes_with_profile(
            &common::metadata_fixture(case),
            page_validation::ValidationProfile::PdfA1a,
            &SafetyLimits::default(),
        );
        match expected {
            Some(rule) => assert!(
                report
                    .failures
                    .iter()
                    .any(|failure| failure.rule_id == rule),
                "{case}: {:#?}",
                report.failures
            ),
            None => assert!(
                report
                    .failures
                    .iter()
                    .all(|failure| failure.rule_id != "PDFA1A-ID-CONFORMANCE-001"),
                "{case}: {:#?}",
                report.failures
            ),
        }
    }

    let mut aliased_a = common::metadata_fixture("id_conformance_alias");
    let from = b"idAlias:conformance=\"B\"";
    let at = aliased_a
        .windows(from.len())
        .position(|window| window == from)
        .expect("aliased conformance");
    aliased_a[at + from.len() - 2] = b'A';
    let aliased_a = page_validation::validate_bytes_with_profile(
        &aliased_a,
        page_validation::ValidationProfile::PdfA1a,
        &SafetyLimits::default(),
    );
    assert!(
        aliased_a
            .failures
            .iter()
            .all(|failure| failure.rule_id != "PDFA1A-ID-CONFORMANCE-001")
    );
    assert!(
        aliased_a
            .failures
            .iter()
            .any(|failure| failure.rule_id == "PDFA1B-ID-CONFORMANCE-PREFIX-001")
    );

    let duplicate = page_validation::validate_bytes_with_profile(
        &common::metadata_fixture("duplicate_identification"),
        page_validation::ValidationProfile::PdfA1a,
        &SafetyLimits::default(),
    );
    assert!(
        duplicate
            .failures
            .iter()
            .any(|failure| failure.rule_id == "PDFA1B-XMP-001")
    );
    assert!(
        duplicate
            .failures
            .iter()
            .all(|failure| failure.rule_id != "PDFA1A-ID-CONFORMANCE-001")
    );
}

#[test]
fn pdfa_1a_identification_cases_match_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let mut config = ReferenceConfig::pinned(executable);
    config.profile = ReferenceProfile::PdfA1a;
    let runner = DifferentialRunner::new(config).expect("pinned veraPDF");
    let conformance_rule = "ISO 19005-1:2005:6.7.11:3";
    for (case, should_fail_conformance, should_fail_schema) in [
        ("accepted_a", false, false),
        ("baseline_b", true, false),
        ("missing_conformance", true, false),
        ("lowercase_conformance", true, false),
        ("id_conformance_alias", true, false),
        ("duplicate_identification", false, true),
    ] {
        let path = env::temp_dir().join(format!(
            "page-pdfa-1a-identification-{case}-{}.pdf",
            std::process::id()
        ));
        fs::write(&path, common::metadata_fixture(case)).expect("write fixture");
        let report = runner.compare_file(&path, &SafetyLimits::default());
        let reference = report.reference_result.expect("veraPDF result");
        let failed = reference
            .failed_rule_ids
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            failed.contains(conformance_rule),
            should_fail_conformance,
            "{case}"
        );
        assert_eq!(
            failed.contains("ISO 19005-1:2005:6.7.11:1"),
            should_fail_schema,
            "{case}"
        );
        fs::remove_file(path).expect("remove fixture");
    }
}

#[test]
fn equivalent_rdf_serializations_match_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let runner = DifferentialRunner::new(ReferenceConfig::pinned(executable)).expect("veraPDF");
    for case in [
        "keywords_qualified_parse_type",
        "title_qualified_expanded",
        "keywords_default_element",
        "reordered_top_level_properties",
        "xmp_latin1_recovery",
        "xmp_ascii_control_reference_recovery",
        "xmp_utf16le_without_bom",
        "xmp_utf32be_without_bom",
        "title_utf8_bom_equivalent",
        "title_utf16_odd_equivalent",
        "localized_language_normalization",
        "creation_date_pdf_z00_equivalent",
        "creation_date_pdf_z00_00_equivalent",
        "creation_date_pdf_unicode_digits_equivalent",
        "creation_date_historic_shifted_equivalent",
        "creation_date_long_fraction_equivalent",
        "extension_duplicate_property_keeps_first",
    ] {
        let path = env::temp_dir().join(format!(
            "page-metadata-equivalent-{case}-{}.pdf",
            std::process::id()
        ));
        fs::write(&path, common::metadata_fixture(case)).expect("write metadata fixture");
        let report = runner.compare_file(&path, &SafetyLimits::default());
        assert_eq!(
            report.classification,
            ComparisonClassification::Agreement,
            "{case}: {report:#?}"
        );
        assert!(
            report
                .reference_result
                .expect("veraPDF result")
                .failed_rule_ids
                .is_empty(),
            "{case}"
        );
        fs::remove_file(path).expect("remove metadata fixture");
    }
    for (case, local_rule, reference_rule) in [
        (
            "xmp_del_reference_preserved",
            "PDFA1B-INFO-PRODUCER-001",
            "ISO 19005-1:2005:6.7.3:7",
        ),
        (
            "extension_container_attribute",
            "PDFA1B-XMP-EXTENSION-CONTAINER-001",
            "ISO 19005-1:2005:6.7.8:2",
        ),
        (
            "creation_date_historic_same_lexical",
            "PDFA1B-INFO-CREATIONDATE-001",
            "ISO 19005-1:2005:6.7.3:1",
        ),
        (
            "id_part_unicode_digit",
            "PDFA1B-XMP-PREDEFINED-VALUE-TYPE-001",
            "ISO 19005-1:2005:6.7.9:3",
        ),
    ] {
        let path = env::temp_dir().join(format!(
            "page-metadata-equivalent-{case}-{}.pdf",
            std::process::id()
        ));
        let bytes = common::metadata_fixture(case);
        fs::write(&path, &bytes).expect("write metadata fixture");
        assert_eq!(common::failure_ids(&bytes), [local_rule.to_owned()].into());
        let report = runner.compare_file(&path, &SafetyLimits::default());
        assert_eq!(
            report.classification,
            ComparisonClassification::BothNoncompliant,
            "{case}: {report:#?}"
        );
        assert_eq!(
            report
                .reference_result
                .expect("veraPDF result")
                .failed_rule_ids
                .into_iter()
                .map(|rule| rule.to_string())
                .collect::<BTreeSet<_>>(),
            [reference_rule.to_owned()].into(),
            "{case}"
        );
        fs::remove_file(path).expect("remove metadata fixture");
    }
}

#[test]
fn rdf_parser_edge_cases_match_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let runner = DifferentialRunner::new(ReferenceConfig::pinned(executable)).expect("veraPDF");
    let empty_metadata_failures = [
        "ISO 19005-1:2005:6.7.11:1",
        "ISO 19005-1:2005:6.7.3:2",
        "ISO 19005-1:2005:6.7.3:3",
        "ISO 19005-1:2005:6.7.3:4",
        "ISO 19005-1:2005:6.7.3:5",
        "ISO 19005-1:2005:6.7.3:6",
        "ISO 19005-1:2005:6.7.3:7",
    ];
    let malformed_metadata_failures = [
        "ISO 19005-1:2005:6.7.9:1",
        "ISO 19005-1:2005:6.7.11:1",
        "ISO 19005-1:2005:6.7.3:2",
        "ISO 19005-1:2005:6.7.3:3",
        "ISO 19005-1:2005:6.7.3:4",
        "ISO 19005-1:2005:6.7.3:5",
        "ISO 19005-1:2005:6.7.3:6",
        "ISO 19005-1:2005:6.7.3:7",
    ];
    for (case, expected_reference_failures) in [
        (
            "empty_xmpmeta_stops_search",
            empty_metadata_failures.as_slice(),
        ),
        ("rdf_cdata_literal", malformed_metadata_failures.as_slice()),
        ("rdf_comment_between_properties", &[]),
        (
            "rdf_nbsp_between_descriptions",
            malformed_metadata_failures.as_slice(),
        ),
        (
            "mismatched_rdf_about",
            malformed_metadata_failures.as_slice(),
        ),
        (
            "rdf_resource_and_value",
            malformed_metadata_failures.as_slice(),
        ),
        (
            "unknown_rdf_property",
            &["ISO 19005-1:2005:6.7.9:2", "ISO 19005-1:2005:6.7.9:3"],
        ),
    ] {
        let path = env::temp_dir().join(format!(
            "page-metadata-rdf-edge-{case}-{}.pdf",
            std::process::id()
        ));
        fs::write(&path, common::metadata_fixture(case)).expect("write metadata fixture");
        let report = runner.compare_file(&path, &SafetyLimits::default());
        assert_eq!(
            report
                .reference_result
                .expect("veraPDF result")
                .failed_rule_ids
                .into_iter()
                .map(|rule| rule.to_string())
                .collect::<BTreeSet<_>>(),
            expected_reference_failures
                .iter()
                .map(|rule| (*rule).to_owned())
                .collect(),
            "{case}"
        );
        fs::remove_file(path).expect("remove metadata fixture");
    }
    for case in [
        "ix_changes_ignored",
        "deprecated_dc_namespace",
        "lang_alt_partial_language",
    ] {
        let path = env::temp_dir().join(format!(
            "page-metadata-rdf-edge-{case}-{}.pdf",
            std::process::id()
        ));
        fs::write(&path, common::metadata_fixture(case)).expect("write metadata fixture");
        let report = runner.compare_file(&path, &SafetyLimits::default());
        assert_eq!(
            report.classification,
            ComparisonClassification::Agreement,
            "{case}: {report:#?}"
        );
        assert!(
            report
                .reference_result
                .expect("veraPDF result")
                .failed_rule_ids
                .is_empty(),
            "{case}"
        );
        fs::remove_file(path).expect("remove metadata fixture");
    }
}

#[test]
fn custom_extension_type_shape_failure_is_reported() {
    let failures = common::failure_ids(&common::metadata_fixture("extension_custom_value_invalid"));
    assert!(failures.contains("PDFA1B-XMP-EXTENSION-PROPERTY-VALUE-SHAPE-001"));
}

#[test]
fn xmp_literal_whitespace_matches_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let path = env::temp_dir().join(format!("page-xmp-whitespace-{}.pdf", std::process::id()));
    fs::write(
        &path,
        common::metadata_fixture("title_whitespace_equivalent"),
    )
    .expect("write whitespace fixture");
    let runner = DifferentialRunner::new(ReferenceConfig::pinned(executable)).expect("veraPDF");
    let report = runner.compare_file(&path, &SafetyLimits::default());
    assert!(
        !common::failure_ids(&fs::read(&path).expect("read whitespace fixture"))
            .contains("PDFA1B-INFO-TITLE-001")
    );
    assert!(
        report
            .reference_result
            .expect("veraPDF result")
            .failed_rule_ids
            .iter()
            .map(ToString::to_string)
            .all(|rule| rule != "ISO 19005-1:2005:6.7.3:2")
    );
    fs::remove_file(path).expect("remove whitespace fixture");
}

#[test]
fn xmp_attribute_whitespace_matches_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let path = env::temp_dir().join(format!(
        "page-xmp-attribute-whitespace-{}.pdf",
        std::process::id()
    ));
    fs::write(&path, common::metadata_fixture("keywords_xmp_whitespace"))
        .expect("write whitespace fixture");
    let runner = DifferentialRunner::new(ReferenceConfig::pinned(executable)).expect("veraPDF");
    let report = runner.compare_file(&path, &SafetyLimits::default());
    assert!(
        common::failure_ids(&fs::read(&path).expect("read whitespace fixture"))
            .contains("PDFA1B-INFO-KEYWORDS-001")
    );
    assert!(
        report
            .reference_result
            .expect("veraPDF result")
            .failed_rule_ids
            .iter()
            .map(ToString::to_string)
            .any(|rule| rule == "ISO 19005-1:2005:6.7.3:5")
    );
    fs::remove_file(path).expect("remove whitespace fixture");
}

#[test]
fn normalized_extension_value_types_match_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let path = env::temp_dir().join(format!(
        "page-extension-rational-value-type-{}.pdf",
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
        "page-invalid-gps-coordinate-{}.pdf",
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

#[test]
fn invalid_extension_xpath_matches_pinned_verapdf_when_opted_in() {
    let Some(executable) = env::var_os("VERAPDF_BIN") else {
        return;
    };
    let path = env::temp_dir().join(format!("page-invalid-xpath-{}.pdf", std::process::id()));
    fs::write(&path, common::metadata_fixture("extension_xpath_invalid"))
        .expect("write XPath fixture");
    let runner = DifferentialRunner::new(ReferenceConfig::pinned(executable)).expect("veraPDF");
    let report = runner.compare_file(&path, &SafetyLimits::default());
    assert!(
        common::failure_ids(&fs::read(&path).expect("read XPath fixture"))
            .contains("PDFA1B-XMP-EXTENSION-PROPERTY-VALUE-SHAPE-001")
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
    fs::remove_file(path).expect("remove XPath fixture");
}
