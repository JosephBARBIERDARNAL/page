use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

const PROFILE_NAMESPACE: &str = "http://www.verapdf.org/ValidationProfile";

#[test]
fn rule_mapping_inventories_match_their_pinned_profiles() {
    let inventories = inventories();
    assert!(!inventories.is_empty(), "no rule-mapping inventories found");

    for (_, inventory) in &inventories {
        assert_eq!(number(&inventory["schema_version"], "schema version"), 1);
        let mapping = object(&inventory["rule_mapping"], "rule_mapping");
        assert_eq!(number(&mapping["schema_version"], "schema version"), 1);

        let profiles = profiles_by_key(mapping);
        let sections = sections_by_key(mapping, &profiles);
        let pinned_predicates = validate_profiles(mapping, &profiles);
        validate_mappings(mapping, &profiles, &sections, &pinned_predicates);
        validate_coverage_gaps(mapping, &profiles, &pinned_predicates);
    }
}

#[test]
fn pdfa_2_3_applicability_is_profile_accurate() {
    let inventory = inventories()
        .into_iter()
        .map(|(_, inventory)| inventory)
        .find(|inventory| {
            inventory["rule_mapping"]["document"]["title"]
                == "PDF/A-2 and PDF/A-3 pinned rule mapping"
        })
        .expect("PDF/A-2/3 rule-mapping inventory");
    let mapping = object(&inventory["rule_mapping"], "rule_mapping");
    let mappings = array(&mapping["mappings"], "mappings");

    for permitted in [
        "PDFA1B-XREF-STREAM-001",
        "PDFA1B-OPTIONAL-CONTENT-001",
        "PDFA1B-TRANSPARENCY-GROUP-001",
    ] {
        assert!(
            mappings
                .iter()
                .all(|item| item["canonical_local_rule_id"] != permitted),
            "permitted feature {permitted} must not be presented as a failure mapping"
        );
    }

    assert_mapping_profiles(
        mappings,
        "PDFA1B-OPTIONAL-CONTENT-NAME-001",
        &["2a", "2b", "2u", "3a", "3b", "3u"],
        "shared_pdfa_2_3",
    );
    assert_mapping_profiles(
        mappings,
        "PDFA1B-TRANSPARENCY-GROUP-CS-001",
        &["2a", "2b", "2u", "3a", "3b", "3u"],
        "shared_pdfa_2_3",
    );
    assert_mapping_profiles(
        mappings,
        "PDFA1B-FILE-SPEC-AF-RELATIONSHIP-001",
        &["3a", "3b", "3u"],
        "pdfa_3",
    );
    assert_mapping_profiles(
        mappings,
        "PDFA1A-UNICODE-MAPPING-001",
        &["2a", "2u", "3a", "3u"],
        "a_or_u",
    );
    assert_mapping_profiles(
        mappings,
        "PDFA1A-TAGGED-DOCUMENT-001",
        &["2a", "3a"],
        "a_only",
    );

}

fn assert_mapping_profiles(
    mappings: &[Value],
    local_rule: &str,
    expected_profiles: &[&str],
    expected_section: &str,
) {
    let matching = mappings
        .iter()
        .filter(|mapping| mapping["canonical_local_rule_id"] == local_rule)
        .collect::<Vec<_>>();
    assert!(!matching.is_empty(), "missing mapping for {local_rule}");
    assert!(
        matching
            .iter()
            .all(|mapping| mapping["section"] == expected_section),
        "{local_rule} is in the wrong applicability section"
    );
    let actual = matching
        .iter()
        .flat_map(|mapping| array(&mapping["applicable_profiles"], "applicable profiles"))
        .map(|profile| string(profile, "applicable profile"))
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected_profiles.iter().copied().collect());
}

fn inventories() -> Vec<(PathBuf, Value)> {
    let fixture_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut paths = fs::read_dir(&fixture_directory)
        .expect("read fixture directory")
        .map(|entry| entry.expect("fixture directory entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("-coverage.json"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| {
            let inventory: Value =
                serde_json::from_slice(&fs::read(&path).expect("read coverage inventory"))
                    .expect("parse coverage inventory");
            inventory
                .get("rule_mapping")
                .is_some_and(Value::is_object)
                .then_some((path, inventory))
        })
        .collect()
}

fn profiles_by_key(mapping: &serde_json::Map<String, Value>) -> BTreeMap<&str, &Value> {
    let mut profiles = BTreeMap::new();
    for profile in array(&mapping["profiles"], "profiles") {
        let key = string(&profile["key"], "profile key");
        assert!(
            profiles.insert(key, profile).is_none(),
            "duplicate profile key {key}"
        );
    }
    assert!(!profiles.is_empty(), "inventory has no profiles");
    profiles
}

fn sections_by_key<'a>(
    mapping: &'a serde_json::Map<String, Value>,
    profiles: &BTreeMap<&str, &Value>,
) -> BTreeMap<&'a str, BTreeSet<&'a str>> {
    let mut sections = BTreeMap::new();
    for section in array(&mapping["applicability_sections"], "applicability sections") {
        let key = string(&section["key"], "section key");
        assert!(
            !string(&section["heading"], "section heading")
                .trim()
                .is_empty()
        );
        assert!(
            !string(&section["description"], "section description")
                .trim()
                .is_empty()
        );
        let applicable = strings(&section["profiles"], "section profiles");
        assert!(!applicable.is_empty(), "section {key} has no profiles");
        assert!(
            applicable
                .iter()
                .all(|profile| profiles.contains_key(profile)),
            "section {key} references an unknown profile"
        );
        assert!(
            sections.insert(key, applicable).is_none(),
            "duplicate section key {key}"
        );
    }
    sections
}

fn validate_profiles<'a>(
    _mapping: &serde_json::Map<String, Value>,
    profiles: &BTreeMap<&'a str, &'a Value>,
) -> BTreeMap<(String, String), (String, String)> {
    let mut all_predicates = BTreeMap::new();
    for (key, profile) in profiles {
        let profile_file = string(&profile["profile_file"], "profile file");
        let bytes = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(profile_file))
            .expect("read pinned profile");
        assert_eq!(
            sha256(&bytes),
            string(&profile["profile_sha256"], "profile digest"),
            "{key} profile digest"
        );
        let xml = roxmltree::Document::parse(
            std::str::from_utf8(&bytes).expect("pinned profile is UTF-8"),
        )
        .expect("parse pinned profile");
        let root = xml.root_element();
        assert_eq!(root.attribute("flavour"), profile["flavour"].as_str());

        let predicate_ids = array(&profile["predicate_ids"], "profile predicate ids")
            .iter()
            .map(|id| string(id, "profile predicate id"))
            .collect::<Vec<_>>();
        assert_eq!(
            predicate_ids.len() as u64,
            number(&profile["predicate_count"], "profile predicate count"),
            "{key} predicate count"
        );
        assert_eq!(
            predicate_ids.iter().copied().collect::<BTreeSet<_>>().len(),
            predicate_ids.len(),
            "{key} duplicate predicate IDs"
        );
        let specification = predicate_ids
            .first()
            .and_then(|id| id.rsplit_once(':'))
            .and_then(|(without_test, _)| without_test.rsplit_once(':'))
            .map(|(specification, _)| specification)
            .expect("profile predicate specification");
        let expected_xml_specification = specification
            .split_once(':')
            .map(|(standard, _)| standard.replace([' ', '-'], "_"))
            .expect("profile specification");

        let rules = xml
            .descendants()
            .filter(|node| node.has_tag_name((PROFILE_NAMESPACE, "rule")))
            .collect::<Vec<_>>();
        assert_eq!(rules.len(), predicate_ids.len(), "{key} XML rule count");
        for rule in rules {
            let id = rule
                .children()
                .find(|child| child.has_tag_name((PROFILE_NAMESPACE, "id")))
                .expect("profile rule id");
            assert_eq!(
                id.attribute("specification"),
                Some(expected_xml_specification.as_str())
            );
            let rule_id = format!(
                "{specification}:{}:{}",
                id.attribute("clause").expect("rule clause"),
                id.attribute("testNumber").expect("rule test number")
            );
            assert!(
                predicate_ids.contains(&rule_id.as_str()),
                "{key} inventory omits {rule_id}"
            );
            let object = rule.attribute("object").expect("rule object").to_owned();
            let predicate = rule
                .children()
                .find(|child| child.has_tag_name((PROFILE_NAMESPACE, "test")))
                .and_then(|node| node.text())
                .expect("rule predicate")
                .trim()
                .to_owned();
            assert!(
                all_predicates
                    .insert(((*key).to_owned(), rule_id.clone()), (object, predicate))
                    .is_none(),
                "duplicate pinned predicate {key} {rule_id}"
            );
        }
    }
    all_predicates
}

fn validate_mappings<'a>(
    mapping: &'a serde_json::Map<String, Value>,
    profiles: &BTreeMap<&'a str, &'a Value>,
    sections: &BTreeMap<&'a str, BTreeSet<&'a str>>,
    pinned_predicates: &BTreeMap<(String, String), (String, String)>,
) {
    let mut unique = BTreeSet::new();
    for item in array(&mapping["mappings"], "mappings") {
        let section = string(&item["section"], "mapping section");
        let section_profiles = sections
            .get(section)
            .expect("mapping references known section");
        let reference_rule = string(&item["verapdf_rule_id"], "veraPDF rule ID");
        let canonical_rule = string(&item["canonical_local_rule_id"], "canonical local rule ID");
        let strength = string(&item["implementation_strength"], "mapping strength");
        assert!(matches!(strength, "exact" | "partial/proxy"));
        assert!(
            !string(&item["semantic_note"], "note").trim().is_empty(),
            "{canonical_rule} has an empty note"
        );
        let applicable = strings(&item["applicable_profiles"], "applicable profiles");
        assert!(!applicable.is_empty(), "{canonical_rule} has no profiles");
        assert!(
            applicable.is_subset(section_profiles),
            "{canonical_rule} does not fit section {section}"
        );
        for profile_key in applicable {
            let profile = profiles
                .get(profile_key)
                .expect("mapping references known profile");
            let (object_type, predicate) = pinned_predicates
                .get(&(profile_key.to_owned(), reference_rule.to_owned()))
                .expect("mapping references a pinned predicate");
            assert_eq!(item["object"], object_type.as_str());
            assert_eq!(item["predicate"], predicate.as_str());
            let reported_rule = reported_local_rule(profile, canonical_rule);
            if profile["remap_local_rule_ids"] == true {
                assert!(
                    reported_rule.starts_with(&format!(
                        "{}-",
                        string(&profile["local_rule_prefix"], "local rule prefix")
                    )),
                    "unexpected reported rule ID {reported_rule}"
                );
            }
            assert!(
                unique.insert((profile_key, reference_rule, canonical_rule)),
                "duplicate mapping for {profile_key} {reference_rule} {canonical_rule}"
            );
        }
    }
}

fn validate_coverage_gaps<'a>(
    mapping: &'a serde_json::Map<String, Value>,
    profiles: &BTreeMap<&'a str, &'a Value>,
    pinned_predicates: &BTreeMap<(String, String), (String, String)>,
) {
    let mapped = array(&mapping["mappings"], "mappings")
        .iter()
        .flat_map(|item| {
            let rule_id = string(&item["verapdf_rule_id"], "veraPDF rule ID");
            array(&item["applicable_profiles"], "applicable profiles")
                .iter()
                .map(move |profile| (string(profile, "applicable profile"), rule_id))
        })
        .collect::<BTreeSet<_>>();
    let mut declared_gaps = BTreeSet::new();
    for gap in array(&mapping["coverage_gaps"], "coverage gaps") {
        let rule_id = string(&gap["verapdf_rule_id"], "coverage-gap rule ID");
        assert!(
            !string(&gap["rationale"], "coverage-gap rationale")
                .trim()
                .is_empty(),
            "{rule_id} has an empty gap rationale"
        );
        for profile in array(&gap["applicable_profiles"], "coverage-gap profiles") {
            let profile = string(profile, "coverage-gap profile");
            assert!(
                profiles.contains_key(profile),
                "unknown gap profile {profile}"
            );
            assert!(
                pinned_predicates.contains_key(&(profile.to_owned(), rule_id.to_owned())),
                "gap {rule_id} is absent from profile {profile}"
            );
            assert!(
                !mapped.contains(&(profile, rule_id)),
                "gap {rule_id} in {profile} also has a local mapping"
            );
            assert!(
                declared_gaps.insert((profile, rule_id)),
                "duplicate gap {profile} {rule_id}"
            );
        }
    }
    let inventoried = mapped
        .union(&declared_gaps)
        .copied()
        .collect::<BTreeSet<_>>();
    let pinned = pinned_predicates
        .keys()
        .map(|(profile, rule_id)| (profile.as_str(), rule_id.as_str()))
        .collect::<BTreeSet<_>>();
    assert_eq!(inventoried, pinned, "predicate catalog is incomplete");
}

fn reported_local_rule(profile: &Value, canonical_rule: &str) -> String {
    if profile["remap_local_rule_ids"] != true {
        return canonical_rule.to_owned();
    }
    let (_, suffix) = canonical_rule
        .split_once('-')
        .expect("canonical rule has a prefix");
    format!(
        "{}-{suffix}",
        string(&profile["local_rule_prefix"], "local rule prefix")
    )
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn object<'a>(value: &'a Value, description: &str) -> &'a serde_json::Map<String, Value> {
    value.as_object().expect(description)
}

fn array<'a>(value: &'a Value, description: &str) -> &'a [Value] {
    value.as_array().map(Vec::as_slice).expect(description)
}

fn string<'a>(value: &'a Value, description: &str) -> &'a str {
    value.as_str().expect(description)
}

fn strings<'a>(value: &'a Value, description: &str) -> BTreeSet<&'a str> {
    array(value, description)
        .iter()
        .map(|item| string(item, description))
        .collect()
}

fn number(value: &Value, description: &str) -> u64 {
    value.as_u64().expect(description)
}
