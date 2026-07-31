use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Command;

use lopdf::xref::XrefType;
use serde_json::Value;
use serde_json::json;
use sha2::{Digest, Sha256};

const INVENTORY_PATH: &str = "tests/fixtures/pdfa-1b-coverage.json";
const PROFILE_PATH: &str = "tests/fixtures/PDFA-1B-1.28.xml";
const DIFFERENTIAL_PATH: &str = "tests/fixtures/verapdf-diff-cases.json";
const EXPECTED_PROFILE_SHA256: &str =
    "1fd81cc8002e089a7597967ee607cd1b49744f29219d08a89f79687657fdc75d";

#[derive(Debug)]
struct AtomicEvidence {
    failed: BTreeSet<String>,
    passed: BTreeSet<String>,
}

#[derive(Debug)]
struct LocalMapping {
    rule_id: String,
    strength: String,
    note: String,
}

#[test]
fn coverage_inventory_matches_the_pinned_profile_and_differential_manifest() {
    let inventory = read_json(INVENTORY_PATH);
    let profile_bytes = fs::read(PROFILE_PATH).expect("read pinned profile");
    assert_eq!(sha256(&profile_bytes), EXPECTED_PROFILE_SHA256);

    let reference = object(&inventory["reference"], "reference");
    assert_eq!(
        string(&reference["product"], "reference product"),
        "veraPDF"
    );
    assert_eq!(string(&reference["version"], "reference version"), "1.28.2");
    assert_eq!(string(&reference["flavour"], "reference flavour"), "1b");
    assert_eq!(
        number(&reference["predicate_count"], "reference predicate count"),
        129
    );
    assert_eq!(
        string(&reference["profile_sha256"], "profile digest"),
        EXPECTED_PROFILE_SHA256
    );

    let profile =
        roxmltree::Document::parse(std::str::from_utf8(&profile_bytes).expect("profile is UTF-8"))
            .expect("parse pinned profile");
    let profile_predicates = profile
        .descendants()
        .filter(|node| node.has_tag_name(("http://www.verapdf.org/ValidationProfile", "rule")))
        .map(|rule| {
            let id = rule
                .children()
                .find(|child| {
                    child.has_tag_name(("http://www.verapdf.org/ValidationProfile", "id"))
                })
                .expect("rule id");
            let rule_id = format!(
                "ISO 19005-1:2005:{}:{}",
                id.attribute("clause").expect("rule clause"),
                id.attribute("testNumber").expect("rule test number")
            );
            let object = rule.attribute("object").expect("rule object").to_owned();
            let predicate = rule
                .children()
                .find(|child| {
                    child.has_tag_name(("http://www.verapdf.org/ValidationProfile", "test"))
                })
                .and_then(|node| node.text())
                .expect("rule predicate")
                .trim()
                .to_owned();
            (rule_id, (object, predicate))
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(profile_predicates.len(), 129);

    let differential = read_json(DIFFERENTIAL_PATH);
    assert_eq!(differential["reference"]["version"], "1.28.2");
    assert_eq!(differential["reference"]["profile"], "1b");
    let atomic = atomic_evidence(&differential);
    let agents = include_str!("../../../AGENTS.md");

    let predicates = array(&inventory["predicates"], "predicates");
    assert_eq!(predicates.len(), 129);
    let inventory_ids = predicates
        .iter()
        .map(|predicate| {
            let rule_id = string(&predicate["verapdf_rule_id"], "veraPDF rule id");
            let (expected_object, expected_predicate) = profile_predicates
                .get(rule_id)
                .unwrap_or_else(|| panic!("{rule_id} is not in the pinned profile"));
            assert_eq!(
                string(&predicate["object"], "profile object"),
                expected_object
            );
            assert_eq!(
                string(&predicate["predicate"], "profile predicate"),
                expected_predicate
            );
            let local_checks = array(&predicate["local_checks"], "local checks");
            assert!(!local_checks.is_empty(), "{rule_id} has no local mapping");
            for local_check in local_checks {
                let local_check = string(local_check, "local check");
                assert!(
                    agents.contains(&format!("| `{local_check}` | `{rule_id}` |")),
                    "{rule_id} mapping to {local_check} is absent from AGENTS.md"
                );
            }
            let strengths = array(
                &predicate["implementation_strength"],
                "implementation strength",
            );
            assert!(!strengths.is_empty(), "{rule_id} has no strength");
            for strength in strengths {
                assert!(
                    matches!(string(strength, "strength"), "exact" | "partial/proxy"),
                    "{rule_id} has an invalid implementation strength"
                );
            }

            assert_state(
                rule_id,
                "applicable_pass",
                &predicate["coverage"]["applicable_pass"],
                Some(&atomic),
            );
            assert_state(
                rule_id,
                "applicable_fail",
                &predicate["coverage"]["applicable_fail"],
                Some(&atomic),
            );
            assert_state(
                rule_id,
                "inapplicable",
                &predicate["coverage"]["inapplicable"],
                None,
            );
            rule_id.to_owned()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        inventory_ids,
        profile_predicates.keys().cloned().collect::<BTreeSet<_>>()
    );

    assert_variant_and_corpus_shape(&inventory);
    assert_completion_policy(&inventory, &differential);
    assert_eq!(
        inventory["low_level_syntax"],
        generated_low_level_syntax_matrix(&inventory),
        "low-level syntax matrix is stale; run the ignored matrix generator"
    );
    assert_eq!(
        inventory["graphical_content"],
        generated_graphical_content_matrix(&inventory),
        "graphical-content matrix is stale; run the ignored matrix generator"
    );
    assert_eq!(
        inventory["document_structure"],
        generated_document_structure_matrix(&inventory),
        "document-structure matrix is stale; run the ignored matrix generator"
    );
}

#[test]
#[ignore = "maintenance generator; its checked output is validated by the ordinary test"]
fn regenerate_coverage_inventory() {
    let mut inventory = read_json(INVENTORY_PATH);
    let differential = read_json(DIFFERENTIAL_PATH);
    let profile_source = fs::read_to_string(PROFILE_PATH).expect("read pinned profile");
    let profile = roxmltree::Document::parse(&profile_source).expect("parse pinned profile");
    let mappings = mapping_table(include_str!("../../../AGENTS.md"));

    let mut failed = BTreeMap::<String, Vec<String>>::new();
    let mut passed = BTreeMap::<String, Vec<String>>::new();
    for (reference, evidence) in atomic_evidence(&differential) {
        for rule_id in evidence.failed {
            failed.entry(rule_id).or_default().push(reference.clone());
        }
        for rule_id in evidence.passed {
            passed.entry(rule_id).or_default().push(reference.clone());
        }
    }

    let always_applicable = BTreeSet::from(["CosDocument", "CosTrailer", "CosXRef", "CosIndirect"]);
    let mut predicates = profile
        .descendants()
        .filter(|node| node.has_tag_name((
            "http://www.verapdf.org/ValidationProfile",
            "rule",
        )))
        .map(|rule| {
            let id = rule
                .children()
                .find(|child| child.has_tag_name((
                    "http://www.verapdf.org/ValidationProfile",
                    "id",
                )))
                .expect("rule id");
            let rule_id = format!(
                "ISO 19005-1:2005:{}:{}",
                id.attribute("clause").expect("rule clause"),
                id.attribute("testNumber").expect("rule test number")
            );
            let object = rule.attribute("object").expect("rule object");
            let local = mappings
                .get(&rule_id)
                .unwrap_or_else(|| panic!("missing mapping for {rule_id}"));
            let evidence = |source: &BTreeMap<String, Vec<String>>,
                            fallback_prefix: &str,
                            note: &str| {
                source.get(&rule_id).map_or_else(
                    || {
                        vec![json!({
                            "kind": "local_regression",
                            "case": format!("{fallback_prefix}:{}", local[0].rule_id),
                            "note": note,
                        })]
                    },
                    |cases| {
                        cases
                            .iter()
                            .map(|case| json!({"kind": "verapdf_delta", "case": case}))
                            .collect::<Vec<_>>()
                    },
                )
            };
            let inapplicable = if always_applicable.contains(object) {
                json!({
                    "representable": false,
                    "reason": format!("{object} is always applicable for this profile predicate."),
                })
            } else {
                json!({
                    "representable": true,
                    "evidence": [{
                        "kind": "fixture",
                        "case": "corpus:typst-pdfa-1b.pdf",
                        "note": "The profile object is absent or outside the selected object graph.",
                    }],
                })
            };
            let applicable_fail = match rule_id.as_str() {
                "ISO 19005-1:2005:6.1.12:7" => json!({
                    "representable": false,
                    "reason": "A failing input needs more than 8,388,607 indirect objects and cannot fit the enforced input/object safety limits.",
                }),
                "ISO 19005-1:2005:6.1.3:2" => json!({
                    "representable": false,
                    "reason": "veraPDF rejects encrypted inputs at its task boundary before emitting this profile rule as a validation delta.",
                }),
                "ISO 19005-1:2005:6.3.2:2" => json!({
                    "representable": false,
                    "reason": "Missing or unsupported subtypes create no veraPDF PDFont object, so the predicate has no failing model instance.",
                }),
                _ => json!({
                    "representable": true,
                    "evidence": evidence(
                        &failed,
                        "local-failure",
                        "Applicable-fail evidence is not yet a central veraPDF delta.",
                    ),
                }),
            };
            let child_text = |name| {
                rule.children()
                    .find(|child| child.tag_name().name() == name)
                    .and_then(|node| node.text())
                    .expect("profile rule child")
                    .trim()
            };
            json!({
                "verapdf_rule_id": rule_id,
                "object": object,
                "description": child_text("description"),
                "predicate": child_text("test"),
                "local_checks": local.iter().map(|mapping| &mapping.rule_id).collect::<Vec<_>>(),
                "implementation_strength": local
                    .iter()
                    .map(|mapping| &mapping.strength)
                    .collect::<BTreeSet<_>>(),
                "mapping_notes": local.iter().map(|mapping| &mapping.note).collect::<Vec<_>>(),
                "coverage": {
                    "applicable_pass": {
                        "representable": true,
                        "evidence": evidence(
                            &passed,
                            "local-baseline",
                            "Applicable-pass evidence is not yet a central veraPDF delta.",
                        ),
                    },
                    "applicable_fail": applicable_fail,
                    "inapplicable": inapplicable,
                },
            })
        })
        .collect::<Vec<_>>();
    predicates.sort_by(|left, right| {
        left["verapdf_rule_id"]
            .as_str()
            .cmp(&right["verapdf_rule_id"].as_str())
    });
    inventory["predicates"] = Value::Array(predicates);
    inventory["corpus"] = generated_corpus(&differential);
    fs::write(
        INVENTORY_PATH,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&inventory).expect("serialize inventory")
        ),
    )
    .expect("write inventory");
}

#[test]
#[ignore = "maintenance generator for the checked low-level syntax matrix"]
fn regenerate_low_level_syntax_matrix() {
    let mut inventory = read_json(INVENTORY_PATH);
    let matrix = generated_low_level_syntax_matrix(&inventory);
    inventory["low_level_syntax"] = matrix;
    fs::write(
        INVENTORY_PATH,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&inventory).expect("serialize inventory")
        ),
    )
    .expect("write inventory");
}

#[test]
#[ignore = "maintenance generator for the checked graphical-content matrix"]
fn regenerate_graphical_content_matrix() {
    let mut inventory = read_json(INVENTORY_PATH);
    inventory["graphical_content"] = generated_graphical_content_matrix(&inventory);
    fs::write(
        INVENTORY_PATH,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&inventory).expect("serialize inventory")
        ),
    )
    .expect("write inventory");
}

#[test]
#[ignore = "maintenance generator for the checked document-structure matrix"]
fn regenerate_document_structure_matrix() {
    let mut inventory = read_json(INVENTORY_PATH);
    inventory["document_structure"] = generated_document_structure_matrix(&inventory);
    fs::write(
        INVENTORY_PATH,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&inventory).expect("serialize inventory")
        ),
    )
    .expect("write inventory");
}

#[test]
#[ignore = "maintenance generator for hash-pinned structural boundary PDFs"]
fn regenerate_structural_boundary_fixtures() {
    let fixtures = Path::new("tests/fixtures");
    let typst = fs::read(fixtures.join("typst-pdfa-1b.pdf")).expect("read Typst baseline");
    let structural = fs::read(fixtures.join("structural.pdf")).expect("read structural baseline");

    let mut header_offset = structural.clone();
    header_offset.insert(0, b'x');
    write_fixture(fixtures, "header-offset.pdf", &header_offset);

    let mut binary_comment = typst.clone();
    binary_comment[10..14].copy_from_slice(b"abcd");
    write_fixture(
        fixtures,
        "header-binary-comment-invalid.pdf",
        &binary_comment,
    );

    let mut trailer_id = typst.clone();
    replace_once(&mut trailer_id, b"/ID[", b"/XX[");
    write_fixture(fixtures, "trailer-id-missing.pdf", &trailer_id);

    let mut post_eof = structural;
    post_eof.extend_from_slice(b"unexpected");
    write_fixture(fixtures, "post-eof-data.pdf", &post_eof);

    let mut odd_hex = fs::read(fixtures.join("structural.pdf")).expect("read structural baseline");
    replace_once(&mut odd_hex, b"/Trapped /False", b"/Foo <ABC>     ");
    write_fixture(fixtures, "odd-hex-string.pdf", &odd_hex);

    let mut invalid_hex =
        fs::read(fixtures.join("structural.pdf")).expect("read structural baseline");
    replace_once(&mut invalid_hex, b"/Trapped /False", b"/Foo <GG>      ");
    write_fixture(
        fixtures,
        "parser-local-discrepancy-invalid-hex.pdf",
        &invalid_hex,
    );

    let mut xref_spacing = typst.clone();
    replace_once(&mut xref_spacing, b"xref\n0 63", b"xref\n0\t63");
    write_fixture(fixtures, "xref-spacing.pdf", &xref_spacing);

    let mut xref_eol = typst.clone();
    replace_once_growing(&mut xref_eol, b"xref\n0 63", b"xref\n\n0 63");
    write_fixture(fixtures, "xref-eol.pdf", &xref_eol);

    let mut stream_length = typst.clone();
    replace_once(&mut stream_length, b"/Length 12", b"/Length 13");
    write_fixture(fixtures, "stream-length-mismatch.pdf", &stream_length);

    let mut stream_eol = typst.clone();
    replace_once(&mut stream_eol, b"stream\n", b"stream\r");
    write_fixture(fixtures, "stream-eol-invalid.pdf", &stream_eol);

    let mut object_syntax = typst.clone();
    replace_once(&mut object_syntax, b"1 0 obj\n", b"1  0obj\n");
    write_fixture(fixtures, "indirect-object-syntax.pdf", &object_syntax);

    let mut xref_stream_document =
        lopdf::Document::load_mem(&typst).expect("parse Typst baseline for xref stream");
    xref_stream_document.reference_table.cross_reference_type = XrefType::CrossReferenceStream;
    let mut xref_stream = Vec::new();
    xref_stream_document
        .save_to(&mut xref_stream)
        .expect("serialize xref-stream boundary fixture");
    write_fixture(fixtures, "xref-stream.pdf", &xref_stream);

    let linearized = fixtures.join("linearized-baseline.pdf");
    let status = Command::new("qpdf")
        .args([
            "--deterministic-id",
            "--linearize",
            "tests/fixtures/typst-pdfa-1b.pdf",
        ])
        .arg(&linearized)
        .status()
        .expect("run qpdf");
    assert!(
        status.success(),
        "qpdf failed to generate linearized fixture"
    );
    let mut mismatched = fs::read(&linearized).expect("read linearized baseline");
    let id = b"556c787363793956315955786e475534314f712b2f773d3d";
    let occurrences = mismatched
        .windows(id.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == id).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(occurrences.len(), 2, "expected first and final trailer IDs");
    mismatched[occurrences[1]] = b'6';
    write_fixture(fixtures, "linearized-id-mismatch.pdf", &mismatched);
}

fn assert_state(
    rule_id: &str,
    state_name: &str,
    state: &Value,
    atomic: Option<&BTreeMap<String, AtomicEvidence>>,
) {
    let representable = state["representable"]
        .as_bool()
        .unwrap_or_else(|| panic!("{rule_id} {state_name} has no representability flag"));
    if !representable {
        assert!(
            state["reason"]
                .as_str()
                .is_some_and(|reason| !reason.trim().is_empty()),
            "{rule_id} {state_name} has no unrepresentable reason"
        );
        return;
    }
    let evidence = array(
        &state["evidence"],
        &format!("{rule_id} {state_name} evidence"),
    );
    assert!(
        !evidence.is_empty(),
        "{rule_id} {state_name} has no evidence"
    );
    for item in evidence {
        let kind = string(&item["kind"], "evidence kind");
        let case = string(&item["case"], "evidence case");
        if kind != "verapdf_delta" {
            continue;
        }
        let atomic = atomic.expect("inapplicable states cannot claim a rule delta");
        let expected = atomic
            .get(case)
            .unwrap_or_else(|| panic!("{rule_id} references unknown atomic case {case}"));
        let ids = if state_name == "applicable_pass" {
            &expected.passed
        } else {
            &expected.failed
        };
        assert!(
            ids.contains(rule_id),
            "{case} does not declare {rule_id} as {state_name}"
        );
    }
}

fn assert_variant_and_corpus_shape(inventory: &Value) {
    for dimension in [
        "direct_indirect",
        "null_wrong_type",
        "inherited",
        "nested",
        "cyclic",
        "malformed",
    ] {
        assert!(
            !array(
                &inventory["variant_matrix"][dimension],
                &format!("variant {dimension}")
            )
            .is_empty(),
            "variant dimension {dimension} has no cases"
        );
    }

    let parser_states = array(&inventory["parser_matrix"], "parser matrix")
        .iter()
        .map(|entry| string(&entry["state"], "parser state"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        parser_states,
        BTreeSet::from([
            "both_reject_malformed",
            "recoverable_invalid_hex",
            "recoverable_xref_eol",
            "recoverable_xref_spacing",
            "reference_parser_discrepancy",
            "valid_recovery",
        ])
    );

    let required_families = BTreeSet::from([
        "annotations",
        "colour",
        "content",
        "document_structure",
        "fonts",
        "forms",
        "metadata",
    ]);
    let covered_families = array(&inventory["integration_corpus"], "integration corpus")
        .iter()
        .flat_map(|entry| array(&entry["families"], "integration families"))
        .map(|family| string(family, "integration family"))
        .collect::<BTreeSet<_>>();
    assert_eq!(covered_families, required_families);

    for entry in array(&inventory["corpus"], "corpus") {
        let fixture = string(&entry["fixture"], "corpus fixture");
        let path = Path::new("tests/fixtures").join(fixture);
        let bytes =
            fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        assert_eq!(
            sha256(&bytes),
            string(&entry["sha256"], "corpus digest"),
            "{fixture} changed byte-for-byte"
        );
        assert!(
            entry["recipe"]
                .as_str()
                .is_some_and(|recipe| !recipe.trim().is_empty()),
            "{fixture} has no generation/provenance recipe"
        );
    }
}

fn assert_completion_policy(inventory: &Value, differential: &Value) {
    let gate = &inventory["completion_gate"];
    let status = string(&gate["status"], "completion status");
    assert!(matches!(status, "developing" | "complete"));
    if std::env::var_os("TAG_REQUIRE_PDFA1B_COMPLETE").is_some() {
        assert_eq!(
            status, "complete",
            "PDF/A-1B release gate requested, but the inventory is still developing"
        );
    }
    let predicates = array(&inventory["predicates"], "predicates");
    let non_delta_states = predicates
        .iter()
        .flat_map(|predicate| {
            ["applicable_pass", "applicable_fail"].map(|state| {
                (
                    string(&predicate["verapdf_rule_id"], "rule id"),
                    state,
                    &predicate["coverage"][state],
                )
            })
        })
        .filter(|(_, _, state)| {
            state["representable"] == true
                && array(&state["evidence"], "state evidence")
                    .iter()
                    .any(|item| item["kind"] != "verapdf_delta")
        })
        .count();
    let partial_predicates = predicates
        .iter()
        .filter(|predicate| {
            array(
                &predicate["implementation_strength"],
                "implementation strength",
            )
            .iter()
            .any(|strength| strength == "partial/proxy")
        })
        .count();
    let incomplete_parser_states = array(&inventory["parser_matrix"], "parser matrix")
        .iter()
        .filter(|entry| entry["representable"] == false)
        .count();
    let expected_coverage_gaps = array(&differential["cases"], "differential cases")
        .iter()
        .filter(|case| case["expected_classification"] == "coverage_gap")
        .count();

    eprintln!(
        "PDF/A-1B completion report: predicates=129, partial/proxy={partial_predicates}, \
         non-delta applicable states={non_delta_states}, incomplete parser states=\
         {incomplete_parser_states}, expected coverage gaps={expected_coverage_gaps}"
    );

    if status == "complete" {
        assert_eq!(partial_predicates, 0, "complete profile has proxy checks");
        assert_eq!(
            non_delta_states, 0,
            "complete profile has applicable states without veraPDF deltas"
        );
        assert_eq!(
            incomplete_parser_states, 0,
            "complete profile has an incomplete parser matrix"
        );
        assert_eq!(
            expected_coverage_gaps, 0,
            "complete profile expects coverage_gap"
        );
        assert_eq!(
            gate["coverage_gap_is_success"], false,
            "complete profile must reject coverage_gap"
        );
    } else {
        assert_eq!(
            gate["coverage_gap_is_success"], true,
            "developing profile preserves the existing differential contract"
        );
    }
}

fn atomic_evidence(differential: &Value) -> BTreeMap<String, AtomicEvidence> {
    let mut result = BTreeMap::new();
    for (key, value) in object(differential, "differential manifest") {
        if !key.starts_with("atomic_") || !key.ends_with("_cases") {
            continue;
        }
        let family = key.trim_start_matches("atomic_").trim_end_matches("_cases");
        for test_case in array(value, key) {
            let name = string(&test_case["name"], "atomic case name");
            let reference = format!("{family}:{name}");
            let evidence = AtomicEvidence {
                failed: strings(
                    &test_case["expected_verapdf_failed_rule_ids"],
                    "failed veraPDF IDs",
                ),
                passed: strings(
                    &test_case["expected_verapdf_passed_rule_ids"],
                    "passed veraPDF IDs",
                ),
            };
            assert!(
                result.insert(reference.clone(), evidence).is_none(),
                "duplicate atomic case {reference}"
            );
        }
    }
    for test_case in array(&differential["cases"], "differential cases") {
        let failed_ids = test_case["expected_verapdf_added_rule_ids"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let passed_ids = test_case["expected_verapdf_passed_rule_ids"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        if failed_ids.is_empty() && passed_ids.is_empty() {
            continue;
        }
        let path = string(&test_case["path"], "differential case path");
        let name = Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .expect("differential case file name");
        let reference = format!("corpus:{name}");
        let evidence = AtomicEvidence {
            failed: failed_ids
                .iter()
                .map(|value| string(value, "added veraPDF ID").to_owned())
                .collect(),
            passed: passed_ids
                .iter()
                .map(|value| string(value, "passed veraPDF ID").to_owned())
                .collect(),
        };
        assert!(
            result.insert(reference.clone(), evidence).is_none(),
            "duplicate differential case {reference}"
        );
    }
    result
}

fn mapping_table(agents: &str) -> BTreeMap<String, Vec<LocalMapping>> {
    let mut mappings = BTreeMap::<String, Vec<LocalMapping>>::new();
    for line in agents.lines() {
        if !line.starts_with("| `PDFA1B-") {
            continue;
        }
        let fields = line.split(" | ").collect::<Vec<_>>();
        if fields.len() != 5 {
            continue;
        }
        let local_rule = fields[0].trim_start_matches("| `").trim_end_matches('`');
        let reference_rule = fields[1].trim_matches('`');
        if !reference_rule.starts_with("ISO 19005-1") {
            continue;
        }
        mappings
            .entry(reference_rule.to_owned())
            .or_default()
            .push(LocalMapping {
                rule_id: local_rule.to_owned(),
                strength: fields[3].to_owned(),
                note: fields[4]
                    .trim_end_matches(" |")
                    .replace("\\|", "|")
                    .to_owned(),
            });
    }
    mappings
}

fn generated_corpus(differential: &Value) -> Value {
    let generated = BTreeSet::from([
        "header-binary-comment-invalid.pdf",
        "header-offset.pdf",
        "indirect-object-syntax.pdf",
        "linearized-baseline.pdf",
        "linearized-id-mismatch.pdf",
        "odd-hex-string.pdf",
        "parser-local-discrepancy-invalid-hex.pdf",
        "post-eof-data.pdf",
        "stream-eol-invalid.pdf",
        "stream-length-mismatch.pdf",
        "trailer-id-missing.pdf",
        "xref-eol.pdf",
        "xref-spacing.pdf",
        "xref-stream.pdf",
    ]);
    let mut names = BTreeSet::new();
    for test_case in array(&differential["cases"], "differential cases") {
        for field in ["path", "reference_baseline"] {
            let Some(path) = test_case[field].as_str() else {
                continue;
            };
            let name = Path::new(path)
                .file_name()
                .and_then(|value| value.to_str())
                .expect("corpus file name");
            names.insert(name.to_owned());
        }
    }
    Value::Array(
        names
            .into_iter()
            .map(|name| {
                let bytes = fs::read(Path::new("tests/fixtures").join(&name))
                    .unwrap_or_else(|error| panic!("read {name}: {error}"));
                let recipe = if name == "typst-pdfa-1b.pdf" {
                    "just typst"
                } else if generated.contains(name.as_str()) {
                    "ignored Rust structural boundary generator"
                } else {
                    "immutable checked-in regression input"
                };
                json!({
                    "fixture": name,
                    "recipe": recipe,
                    "sha256": sha256(&bytes),
                })
            })
            .collect(),
    )
}

fn generated_low_level_syntax_matrix(inventory: &Value) -> Value {
    let predicates = array(&inventory["predicates"], "predicates");
    let mut entries = predicates
        .iter()
        .filter(|predicate| {
            string(&predicate["verapdf_rule_id"], "veraPDF rule id")
                .starts_with("ISO 19005-1:2005:6.1.")
        })
        .map(|predicate| {
            let rule_id = string(&predicate["verapdf_rule_id"], "veraPDF rule id");
            let provenance_class = low_level_provenance_class(rule_id);
            let milestone_required = provenance_class != "content_or_embedded_program";
            let strengths = array(
                &predicate["implementation_strength"],
                "implementation strength",
            );
            if milestone_required {
                assert!(
                    strengths
                        .iter()
                        .all(|strength| string(strength, "strength") == "exact"),
                    "{rule_id} is in the low-level milestone but is not exact"
                );
            }
            json!({
                "verapdf_rule_id": rule_id,
                "object": predicate["object"],
                "predicate": predicate["predicate"],
                "milestone_required": milestone_required,
                "provenance_class": provenance_class,
                "applicability": predicate["mapping_notes"],
                "implementation_path": predicate["local_checks"],
                "recovery_model": low_level_recovery_model(provenance_class),
                "implementation_strength": predicate["implementation_strength"],
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        string(&left["verapdf_rule_id"], "left rule id")
            .cmp(string(&right["verapdf_rule_id"], "right rule id"))
    });
    let required_count = entries
        .iter()
        .filter(|entry| entry["milestone_required"] == true)
        .count();
    json!({
        "status": "complete",
        "source": "veraPDF 1.28.2 PDF/A-1B profile",
        "profile_predicate_count": predicates.len(),
        "inventoried_clause_6_1_predicate_count": entries.len(),
        "required_predicate_count": required_count,
        "policy": "Every clause 6.1 predicate is inventoried. The milestone requires exact behavior for raw file syntax and selected COS objects; content execution and embedded-program populations remain governed by their owning coverage families.",
        "predicates": entries,
    })
}

fn generated_graphical_content_matrix(inventory: &Value) -> Value {
    let shared = BTreeSet::from(["ISO 19005-1:2005:6.1.10:2", "ISO 19005-1:2005:6.1.12:9"]);
    let mut entries = array(&inventory["predicates"], "predicates")
        .iter()
        .filter(|predicate| {
            let rule_id = string(&predicate["verapdf_rule_id"], "veraPDF rule id");
            rule_id.starts_with("ISO 19005-1:2005:6.2.") || shared.contains(rule_id)
        })
        .map(|predicate| {
            let rule_id = string(&predicate["verapdf_rule_id"], "veraPDF rule id");
            let strengths = array(
                &predicate["implementation_strength"],
                "implementation strength",
            );
            assert!(
                strengths
                    .iter()
                    .all(|strength| string(strength, "strength") == "exact"),
                "{rule_id} is in the graphical-content milestone but is not exact"
            );
            json!({
                "verapdf_rule_id": rule_id,
                "object": predicate["object"],
                "predicate": predicate["predicate"],
                "implementation_path": predicate["local_checks"],
                "applicability": predicate["mapping_notes"],
                "implementation_strength": predicate["implementation_strength"],
                "coverage": predicate["coverage"],
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        string(&left["verapdf_rule_id"], "left rule id")
            .cmp(string(&right["verapdf_rule_id"], "right rule id"))
    });
    assert_eq!(entries.len(), 21, "graphical-content predicate count");
    json!({
        "status": "complete",
        "source": "veraPDF 1.28.2 PDF/A-1B profile",
        "clause_6_2_predicate_count": 19,
        "shared_clause_6_1_predicate_count": 2,
        "exact_predicate_count": 21,
        "policy": "One bounded page/Form execution model supplies every clause 6.2 graphical-content population plus inline-image LZW and DeviceN component evidence.",
        "predicates": entries,
    })
}

/// The pinned predicates whose applicability genuinely "begins at the
/// catalog": evaluating them requires resolving the trailer `/Root` and
/// walking a structure rooted there (`Names`, `OCProperties`, `AcroForm`, or
/// an action's file specification), as opposed to a predicate whose
/// `object` happens to be `CosDocument`/`CosTrailer`/`CosXRef` but whose
/// applicability is raw file bytes, the trailer directly, or a
/// whole-document fact independent of `/Root` (the file header, the
/// post-EOF byte count, the applicable trailer's `ID`, linearized-trailer
/// agreement, xref-subsection spacing/EOL, `/Encrypt` presence, the
/// indirect-object count, or xref-stream presence) — those are inventoried
/// by the low-level-syntax milestone instead, per
/// `low_level_provenance_class`.
fn document_structure_catalog_rooted_rule_ids() -> BTreeSet<&'static str> {
    BTreeSet::from([
        "ISO 19005-1:2005:6.1.11:1",
        "ISO 19005-1:2005:6.1.11:2",
        "ISO 19005-1:2005:6.1.13:1",
        "ISO 19005-1:2005:6.6.2:2",
        "ISO 19005-1:2005:6.6.2:3",
        "ISO 19005-1:2005:6.7.2:1",
        "ISO 19005-1:2005:6.9:1",
    ])
}

fn document_structure_traversal_origin(rule_id: &str) -> &'static str {
    match rule_id {
        "ISO 19005-1:2005:6.1.11:1" => {
            "catalog::resolve_catalog -> Names -> EmbeddedFiles name tree (document_features::inspect_name_tree, generalized to track its own root reference for cycle detection), independently reachable a second way through a GoToR/SubmitForm action's /F entry (actions::inspect_action_value -> file_spec::inspect, confirmed against veraPDF 1.28.2 to instantiate the same CosFileSpecification object either way)"
        }
        "ISO 19005-1:2005:6.1.11:2" => {
            "catalog::resolve_catalog -> Names -> EmbeddedFiles key presence (document_features::inspect)"
        }
        "ISO 19005-1:2005:6.1.13:1" => {
            "catalog::resolve_catalog -> OCProperties key presence (document_features::inspect)"
        }
        "ISO 19005-1:2005:6.6.2:2" => {
            "catalog::resolve_catalog -> AcroForm -> Fields tree, recursive Kids with a top-level-vs-child /T distinction matching veraPDF (actions::inspect_acro_form / inspect_field)"
        }
        "ISO 19005-1:2005:6.6.2:3" => {
            "catalog::resolve_catalog -> AA key presence (actions::inspect)"
        }
        "ISO 19005-1:2005:6.7.2:1" => {
            "catalog::resolve_catalog -> Metadata stream (model::normalize -> inspect_catalog_metadata)"
        }
        "ISO 19005-1:2005:6.9:1" => {
            "catalog::resolve_catalog -> AcroForm -> NeedAppearances (forms::inspect_acro_form)"
        }
        _ => unreachable!("rule id not in the catalog-rooted set"),
    }
}

/// Documents the acceptance-criteria question "every document-structure
/// predicate has a documented veraPDF-backed applicability and traversal
/// model" precisely, rather than by assertion: every pinned predicate whose
/// evaluation requires walking from the trailer `/Root` is listed with the
/// exact shared-traversal call path that supplies it. Separately records
/// the (confirmed, not assumed) fact that neither the page tree nor a
/// general name-tree node owns any predicate of its own in this profile —
/// no `PDPage`, `PDPagesTreeNode`, or name-tree-node object exists among
/// the 129 pinned predicates — so both are pure shared infrastructure
/// (`page_tree::collect_pages`, the name-tree walker) whose correctness is
/// verified indirectly through every downstream family that depends on it.
fn generated_document_structure_matrix(inventory: &Value) -> Value {
    let catalog_rooted = document_structure_catalog_rooted_rule_ids();
    let mut entries = array(&inventory["predicates"], "predicates")
        .iter()
        .filter(|predicate| {
            catalog_rooted.contains(string(&predicate["verapdf_rule_id"], "veraPDF rule id"))
        })
        .map(|predicate| {
            let rule_id = string(&predicate["verapdf_rule_id"], "veraPDF rule id").to_owned();
            let traversal_origin = document_structure_traversal_origin(&rule_id);
            json!({
                "verapdf_rule_id": rule_id,
                "object": predicate["object"],
                "predicate": predicate["predicate"],
                "implementation_path": predicate["local_checks"],
                "applicability": predicate["mapping_notes"],
                "implementation_strength": predicate["implementation_strength"],
                "traversal_origin": traversal_origin,
                "coverage": predicate["coverage"],
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        string(&left["verapdf_rule_id"], "left rule id")
            .cmp(string(&right["verapdf_rule_id"], "right rule id"))
    });
    assert_eq!(
        entries.len(),
        catalog_rooted.len(),
        "catalog-rooted predicate count"
    );
    json!({
        "status": "complete",
        "source": "veraPDF 1.28.2 PDF/A-1B profile",
        "catalog_rooted_predicate_count": entries.len(),
        "page_tree_owned_predicate_count": 0,
        "name_tree_owned_predicate_count": 0,
        "policy": "Applicability 'begins at the catalog' means evaluating the predicate requires resolving the trailer /Root and walking a structure rooted there (Names, OCProperties, AcroForm, or an action's file specification). A predicate whose object is CosDocument/CosTrailer/CosXRef but whose applicability is raw file bytes, the trailer directly, or a whole-document xref/object-count fact independent of /Root is excluded here and is instead inventoried by the low-level-syntax milestone. Neither the page tree nor a general name-tree node owns a predicate of its own anywhere in this profile: no PDPage, PDPagesTreeNode, or name-tree-node object exists among the 129 pinned predicates. Both are pure shared traversal substrate (catalog::resolve_catalog, page_tree::collect_pages, and the generalized name-tree walker in document_features.rs) feeding the downstream annotation, action, form, font, colour, and content predicate families instead of owning predicates themselves; their correctness (cycle detection, /Type strictness matching veraPDF's fatal parse exception, direct-dictionary support) is verified by page_tree.rs's and document_features.rs's own unit tests plus every downstream family's atomic and differential coverage, not by a predicate of their own.",
        "downstream_families_fed_by_page_tree": [
            "annotations (PDAnnot, PDWidgetAnnot)",
            "actions (PDAction, PDNamedAction)",
            "forms (PDAcroForm field walk, PDFormField)",
            "content execution (Op_Undefined, Op_q_gsave, PDGroup, PDExtGState, colour spaces)",
            "fonts (PDFont, PDSimpleFont, PDCIDFont, PDType0Font, PDType1Font, PDTrueTypeFont, Glyph, CMapFile)"
        ],
        "predicates": entries,
    })
}

fn low_level_provenance_class(rule_id: &str) -> &'static str {
    match rule_id {
        "ISO 19005-1:2005:6.1.10:2"
        | "ISO 19005-1:2005:6.1.12:8"
        | "ISO 19005-1:2005:6.1.12:9"
        | "ISO 19005-1:2005:6.1.12:10" => "content_or_embedded_program",
        "ISO 19005-1:2005:6.1.2:1"
        | "ISO 19005-1:2005:6.1.2:2"
        | "ISO 19005-1:2005:6.1.3:1"
        | "ISO 19005-1:2005:6.1.3:3"
        | "ISO 19005-1:2005:6.1.3:4"
        | "ISO 19005-1:2005:6.1.4:1"
        | "ISO 19005-1:2005:6.1.4:2"
        | "ISO 19005-1:2005:6.1.6:1"
        | "ISO 19005-1:2005:6.1.6:2"
        | "ISO 19005-1:2005:6.1.7:1"
        | "ISO 19005-1:2005:6.1.7:2"
        | "ISO 19005-1:2005:6.1.8:1" => "raw_file",
        _ => "selected_cos_object",
    }
}

fn low_level_recovery_model(provenance_class: &str) -> &'static str {
    match provenance_class {
        "raw_file" => {
            "Evaluate original byte spans and revision-selected syntax; recover only oracle-pinned lexical or xref forms."
        }
        "selected_cos_object" => {
            "Evaluate the active revision's modeled value using pinned duplicate-key, null, direct/indirect, and name-decoding semantics."
        }
        "content_or_embedded_program" => {
            "Use the owning bounded content or embedded-program inspector; raw COS provenance is not the source of partial coverage."
        }
        _ => unreachable!("known provenance class"),
    }
}

fn replace_once(bytes: &mut [u8], from: &[u8], to: &[u8]) {
    assert_eq!(from.len(), to.len(), "same-length replacement required");
    let offset = bytes
        .windows(from.len())
        .position(|window| window == from)
        .unwrap_or_else(|| panic!("could not find {:?}", String::from_utf8_lossy(from)));
    bytes[offset..offset + to.len()].copy_from_slice(to);
}

fn replace_once_growing(bytes: &mut Vec<u8>, from: &[u8], to: &[u8]) {
    let offset = bytes
        .windows(from.len())
        .position(|window| window == from)
        .unwrap_or_else(|| panic!("could not find {:?}", String::from_utf8_lossy(from)));
    bytes.splice(offset..offset + from.len(), to.iter().copied());
}

fn write_fixture(fixtures: &Path, name: &str, bytes: &[u8]) {
    fs::write(fixtures.join(name), bytes).unwrap_or_else(|error| panic!("write {name}: {error}"));
}

fn read_json(path: &str) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap_or_else(|error| panic!("read {path}: {error}")))
        .unwrap_or_else(|error| panic!("parse {path}: {error}"))
}

fn strings(value: &Value, label: &str) -> BTreeSet<String> {
    array(value, label)
        .iter()
        .map(|value| string(value, label).to_owned())
        .collect()
}

fn object<'a>(value: &'a Value, label: &str) -> &'a serde_json::Map<String, Value> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("{label} must be an object"))
}

fn array<'a>(value: &'a Value, label: &str) -> &'a Vec<Value> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{label} must be an array"))
}

fn string<'a>(value: &'a Value, label: &str) -> &'a str {
    value
        .as_str()
        .unwrap_or_else(|| panic!("{label} must be a string"))
}

fn number(value: &Value, label: &str) -> u64 {
    value
        .as_u64()
        .unwrap_or_else(|| panic!("{label} must be an unsigned integer"))
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
