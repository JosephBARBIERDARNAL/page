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
    "1c8e6bbb1134f611f768243babf2c17a88069334144441035d274de1b4b64a89";

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
    assert_eq!(string(&reference["version"], "reference version"), "1.30.2");
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
    assert_eq!(differential["reference"]["version"], "1.30.2");
    assert_eq!(differential["reference"]["profile"], "1b");
    assert_eq!(
        inventory["checked_in_mutation_manifest"]["path"],
        DIFFERENTIAL_PATH
    );
    assert_eq!(
        inventory["checked_in_mutation_manifest"]["field"],
        "checked_in_mutations"
    );
    assert_eq!(
        number(
            &inventory["checked_in_mutation_manifest"]["representative_count"],
            "checked-in mutation count"
        ),
        array(
            &differential["checked_in_mutations"],
            "checked-in mutations"
        )
        .len() as u64
    );
    let atomic = atomic_evidence(&differential);
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
            let mapping_notes = array(&predicate["mapping_notes"], "mapping notes");
            assert_eq!(
                local_checks.len(),
                mapping_notes.len(),
                "{rule_id} must have one mapping note per local check"
            );
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

    let mutation_ids = array(
        &differential["checked_in_mutations"],
        "checked-in mutations",
    )
    .iter()
    .map(|mutation| string(&mutation["local_rule_id"], "mutation local rule id"))
    .collect::<BTreeSet<_>>();
    let exception_ids = array(
        &inventory["checked_in_mutation_exceptions"],
        "checked-in mutation exceptions",
    )
    .iter()
    .map(|exception| {
        let kind = string(&exception["kind"], "mutation exception kind");
        assert!(
            matches!(
                kind,
                "non_representable" | "compound_parser_case" | "upstream_reference_exception"
            ),
            "invalid mutation exception kind: {kind}"
        );
        assert!(
            !string(&exception["fixture"], "mutation exception fixture").is_empty(),
            "mutation exception has no fixture"
        );
        assert!(
            !string(&exception["rationale"], "mutation exception rationale")
                .trim()
                .is_empty(),
            "mutation exception has no rationale"
        );
        assert!(
            !string(
                &exception["reference_rule_id"],
                "mutation exception reference rule id"
            )
            .trim()
            .is_empty(),
            "mutation exception has no expected veraPDF rule delta"
        );
        let classification = string(
            &exception["expected_classification"],
            "mutation exception classification",
        );
        assert!(
            matches!(
                classification,
                "reference_parser_discrepancy" | "both_noncompliant" | "resource_limit_boundary"
            ),
            "invalid mutation exception classification: {classification}"
        );
        string(
            &exception["local_rule_id"],
            "mutation exception local rule id",
        )
    })
    .collect::<BTreeSet<_>>();
    let inapplicable_ids = array(
        &inventory["checked_in_mutation_inapplicable"],
        "checked-in mutation inapplicable cases",
    )
    .iter()
    .map(|case| {
        assert!(
            !string(&case["fixture"], "inapplicable fixture").is_empty(),
            "inapplicable mutation case has no fixture"
        );
        assert!(
            !string(&case["rationale"], "inapplicable rationale")
                .trim()
                .is_empty(),
            "inapplicable mutation case has no rationale"
        );
        string(&case["local_rule_id"], "inapplicable local rule id")
    })
    .collect::<BTreeSet<_>>();
    assert!(
        mutation_ids.is_disjoint(&exception_ids)
            && mutation_ids.is_disjoint(&inapplicable_ids)
            && exception_ids.is_disjoint(&inapplicable_ids),
        "a local rule cannot be both a mutation, exception, or inapplicable-only case"
    );
    assert_eq!(
        mutation_ids
            .union(&exception_ids)
            .chain(inapplicable_ids.iter())
            .cloned()
            .collect::<BTreeSet<_>>(),
        predicates
            .iter()
            .flat_map(|predicate| {
                array(&predicate["local_checks"], "local checks")
                    .iter()
                    .map(|local| string(local, "local check"))
                    .collect::<Vec<_>>()
            })
            .collect::<BTreeSet<_>>(),
        "every mapped local predicate needs a checked-in mutation or an explicit exception"
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
    assert_eq!(
        inventory["font"],
        generated_font_matrix(&inventory),
        "font matrix is stale; run the ignored matrix generator"
    );
}

#[test]
#[ignore = "maintenance generator; its checked output is validated by the ordinary test"]
fn regenerate_coverage_inventory() {
    let mut inventory = read_json(INVENTORY_PATH);
    let differential = read_json(DIFFERENTIAL_PATH);
    let profile_source = fs::read_to_string(PROFILE_PATH).expect("read pinned profile");
    let profile = roxmltree::Document::parse(&profile_source).expect("parse pinned profile");
    let mappings = inventory_mappings(&inventory);

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
#[ignore = "maintenance generator for the checked font matrix"]
fn regenerate_font_matrix() {
    let mut inventory = read_json(INVENTORY_PATH);
    inventory["font"] = generated_font_matrix(&inventory);
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
            "--newline-before-endstream",
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
    let id = b"5a7030693074356444525963496d46574d616a4a6b413d3d";
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
        .expect("state has a representability flag");
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
        let expected = atomic.get(case).expect("known atomic case");
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
                inventory
                    .get("variant_matrix")
                    .and_then(|matrix| matrix.get(dimension))
                    .expect("variant matrix dimension"),
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
        let bytes = fs::read(&path).expect("read corpus fixture");
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
    if std::env::var_os("PAGE_REQUIRE_PDFA1B_COMPLETE").is_some() {
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
                    predicate
                        .get("coverage")
                        .and_then(|coverage| coverage.get(state))
                        .expect("predicate coverage state"),
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

fn inventory_mappings(inventory: &Value) -> BTreeMap<String, Vec<LocalMapping>> {
    let mut mappings = BTreeMap::<String, Vec<LocalMapping>>::new();
    for predicate in array(&inventory["predicates"], "predicates") {
        let reference_rule = string(&predicate["verapdf_rule_id"], "veraPDF rule id");
        let local_checks = array(&predicate["local_checks"], "local checks");
        let notes = array(&predicate["mapping_notes"], "mapping notes");
        let strengths = array(
            &predicate["implementation_strength"],
            "implementation strength",
        );
        assert_eq!(
            local_checks.len(),
            notes.len(),
            "{reference_rule} must have one mapping note per local check"
        );
        assert_eq!(
            strengths.len(),
            1,
            "{reference_rule} must currently have one shared implementation strength"
        );
        let strength = string(
            strengths.first().expect("implementation strength"),
            "implementation strength",
        );
        for (local_check, note) in local_checks.iter().zip(notes) {
            mappings
                .entry(reference_rule.to_owned())
                .or_default()
                .push(LocalMapping {
                    rule_id: string(local_check, "local check").to_owned(),
                    strength: strength.to_owned(),
                    note: string(note, "mapping note").to_owned(),
                });
        }
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
                    .expect("read generated corpus fixture");
                let recipe = if Path::new("tests/fixtures")
                    .join(&name)
                    .with_extension("typ")
                    .is_file()
                {
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
        "source": "veraPDF 1.30.2 PDF/A-1B profile",
        "profile_predicate_count": predicates.len(),
        "inventoried_clause_6_1_predicate_count": entries.len(),
        "required_predicate_count": required_count,
        "policy": "Every clause 6.1 predicate is inventoried. The milestone requires exact behavior for raw file syntax and selected COS objects; content execution and embedded-program populations remain governed by their owning coverage families.",
        "predicates": entries,
    })
}

fn generated_graphical_content_matrix(inventory: &Value) -> Value {
    let shared = BTreeSet::from([
        "ISO 19005-1:2005:6.1.10:2",
        "ISO 19005-1:2005:6.1.12:8",
        "ISO 19005-1:2005:6.1.12:9",
    ]);
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
    assert_eq!(entries.len(), 22, "graphical-content predicate count");
    json!({
        "status": "complete",
        "source": "veraPDF 1.30.2 PDF/A-1B profile",
        "clause_6_2_predicate_count": 19,
        "shared_clause_6_1_predicate_count": 3,
        "exact_predicate_count": 22,
        "policy": "One bounded content executor covers page, Form, appearance, selected Pattern, and rendered Type3 paths and supplies every clause 6.2 graphical-content population plus inline-image LZW, graphics-state nesting, and DeviceN component evidence.",
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

fn document_structure_traversal_origin(rule_id: &str) -> Option<&'static str> {
    match rule_id {
        "ISO 19005-1:2005:6.1.11:1" => Some(
            "catalog::resolve_catalog -> Names -> EmbeddedFiles name tree (document_features::inspect_name_tree, generalized to track its own root reference for cycle detection), independently reachable a second way through a GoToR/SubmitForm action's /F entry (actions::inspect_action_value -> file_spec::inspect, confirmed against veraPDF 1.30.2 to instantiate the same CosFileSpecification object either way)",
        ),
        "ISO 19005-1:2005:6.1.11:2" => Some(
            "catalog::resolve_catalog -> Names -> EmbeddedFiles key presence (document_features::inspect)",
        ),
        "ISO 19005-1:2005:6.1.13:1" => Some(
            "catalog::resolve_catalog -> OCProperties key presence (document_features::inspect)",
        ),
        "ISO 19005-1:2005:6.6.2:2" => Some(
            "catalog::resolve_catalog -> AcroForm -> Fields tree, recursive Kids with a top-level-vs-child /T distinction matching veraPDF (actions::inspect_acro_form / inspect_field)",
        ),
        "ISO 19005-1:2005:6.6.2:3" => {
            Some("catalog::resolve_catalog -> AA key presence (actions::inspect)")
        }
        "ISO 19005-1:2005:6.7.2:1" => Some(
            "catalog::resolve_catalog -> Metadata stream (model::normalize -> inspect_catalog_metadata)",
        ),
        "ISO 19005-1:2005:6.9:1" => Some(
            "catalog::resolve_catalog -> AcroForm -> NeedAppearances (forms::inspect_acro_form)",
        ),
        _ => None,
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
            let traversal_origin = document_structure_traversal_origin(&rule_id)
                .expect("catalog-rooted rule has traversal origin");
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
    let family_accounting = generated_predicate_family_accounting(inventory, &catalog_rooted);
    json!({
        "status": "complete",
        "source": "veraPDF 1.30.2 PDF/A-1B profile",
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
        "family_accounting": family_accounting,
    })
}

/// The 20 distinct veraPDF rule ids implemented by `font_embedding.rs`
/// (§§6.3.2-6.3.7 plus the shared §6.1.12:10 CID-range limit); one of them,
/// `ISO 19005-1:2005:6.3.5:1`, is shared by two local checks
/// (`PDFA1B-TYPE1-GLYPH-PRESENCE-001` and
/// `PDFA1B-TRUETYPE-GLYPH-PRESENCE-001`), so this set has 20 members for 21
/// local rule ids. `PDFA1B-CMAP-CID-RANGE-001` is deliberately absent: the
/// coverage inventory records it as a local bounded precondition supporting
/// the 6.3.5:1 glyph-presence predicate, with no veraPDF rule id of its own,
/// so it cannot appear in the profile-keyed `predicates` array.
fn font_predicate_rule_ids() -> BTreeSet<&'static str> {
    BTreeSet::from([
        "ISO 19005-1:2005:6.1.12:10",
        "ISO 19005-1:2005:6.3.2:1",
        "ISO 19005-1:2005:6.3.2:2",
        "ISO 19005-1:2005:6.3.2:3",
        "ISO 19005-1:2005:6.3.2:4",
        "ISO 19005-1:2005:6.3.2:5",
        "ISO 19005-1:2005:6.3.2:6",
        "ISO 19005-1:2005:6.3.2:7",
        "ISO 19005-1:2005:6.3.3.1:1",
        "ISO 19005-1:2005:6.3.3.2:1",
        "ISO 19005-1:2005:6.3.3.3:1",
        "ISO 19005-1:2005:6.3.3.3:2",
        "ISO 19005-1:2005:6.3.4:1",
        "ISO 19005-1:2005:6.3.5:1",
        "ISO 19005-1:2005:6.3.5:2",
        "ISO 19005-1:2005:6.3.5:3",
        "ISO 19005-1:2005:6.3.6:1",
        "ISO 19005-1:2005:6.3.7:1",
        "ISO 19005-1:2005:6.3.7:2",
        "ISO 19005-1:2005:6.3.7:3",
    ])
}

/// The shared content-execution routes that populate
/// `ContentExecutionSummary::fonts` (the population every font predicate
/// below except `CMAP-MAX-CID-001` evaluates against): page content, Form XObjects
/// invoked via `Do` (including the first used Type0 descendant), annotation
/// `/AP`/`/N` appearance streams (every button-Widget appearance state, not
/// only the `/AS`-selected one), tiling Pattern content selected via
/// `cs`/`scn`, and Type3 `/CharProcs` glyph descriptions for rendered
/// glyphs. The three additional source families were each confirmed live against
/// veraPDF 1.30.2 (a font used only there still populates a `PDFont`
/// object) before this milestone added them; see the
/// `font_content_source_*` atomic and differential cases.
const FONT_CONTENT_SOURCES: &[&str] = &[
    "page content (content_support::ContentExecutor::execute_page/execute_contents)",
    "Form XObjects invoked via Do, including the first used Type0 descendant (ContentExecutor::execute_xobject)",
    "annotation /AP /N, /R, and /D appearance streams, including every appearance state (ContentExecutor::execute_annotation_appearances)",
    "tiling Pattern content selected via cs/scn and actually painted (ContentExecutor::execute_pattern)",
    "Type3 /CharProcs glyph descriptions for rendered glyphs (ContentExecutor::execute_type3_glyphs)",
];

fn font_predicate_content_sources(rule_id: &str) -> &'static [&'static str] {
    match rule_id {
        // Scans every embedded CMap object reachable in the parsed object
        // graph directly (inspect_all_embedded_cmap_cids), independent of
        // which fonts are used or where -- not fed by content execution at
        // all, so it does not share the other predicates' content-source
        // list.
        "ISO 19005-1:2005:6.1.12:10" => &[
            "every embedded CMap object in the parsed document (font_embedding::inspect_all_embedded_cmap_cids), independent of content execution",
        ],
        _ => FONT_CONTENT_SOURCES,
    }
}

/// Documents the acceptance-criteria question "font discovery includes
/// every veraPDF-recognized page, Form, appearance, Pattern, and Type3
/// content path" precisely, rather than by assertion: every pinned font
/// predicate is listed with the exact set of content-discovery routes that
/// feed the shared `ContentExecutionSummary::fonts` population it evaluates
/// against.
fn generated_font_matrix(inventory: &Value) -> Value {
    let rule_ids = font_predicate_rule_ids();
    let mut entries = array(&inventory["predicates"], "predicates")
        .iter()
        .filter(|predicate| {
            rule_ids.contains(string(&predicate["verapdf_rule_id"], "veraPDF rule id"))
        })
        .map(|predicate| {
            let rule_id = string(&predicate["verapdf_rule_id"], "veraPDF rule id").to_owned();
            let content_sources = font_predicate_content_sources(&rule_id);
            json!({
                "verapdf_rule_id": rule_id,
                "object": predicate["object"],
                "predicate": predicate["predicate"],
                "implementation_path": predicate["local_checks"],
                "applicability": predicate["mapping_notes"],
                "implementation_strength": predicate["implementation_strength"],
                "content_sources": content_sources,
                "coverage": predicate["coverage"],
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        string(&left["verapdf_rule_id"], "left rule id")
            .cmp(string(&right["verapdf_rule_id"], "right rule id"))
    });
    assert_eq!(entries.len(), rule_ids.len(), "font predicate count");
    json!({
        "status": "complete",
        "source": "veraPDF 1.30.2 PDF/A-1B profile",
        "predicate_count": entries.len(),
        "local_only_precondition_rules": [
            "PDFA1B-CMAP-CID-RANGE-001"
        ],
        "policy": "The 20 pinned font predicates below (§§6.3.2-6.3.7 plus the shared §6.1.12:10 CID-range limit; ISO 19005-1:2005:6.3.5:1 covers two local checks) are evaluated against font_embedding::Scanner::uses, a single population fed by every content-discovery route veraPDF itself recognizes: page content, Form XObjects invoked via Do (including the first used Type0 descendant), annotation appearance streams (every button-Widget state, not only the /AS-selected one), Pattern content actually selected via cs/scn, and Type3 CharProcs for rendered glyphs. CMAP-MAX-CID-001 is the one exception, scanning every embedded CMap object directly instead. PDFA1B-CMAP-CID-RANGE-001 has no veraPDF rule id of its own (a local bounded precondition for the 6.3.5:1 glyph-presence predicate over CID-keyed fonts) and so is listed by name only, not as a profile-keyed predicate entry.",
        "predicates": entries,
    })
}

/// Assigns every one of the 129 pinned predicates to exactly one owning
/// family, driven by its local rule id(s) (which map 1:1 to the
/// `validate_document` dispatch function that implements it — see
/// `crates/page_validation/src/validation.rs`), and asserts the counts sum
/// to 129 with no leftover. This directly answers "is every predicate,
/// not just the 7 catalog-rooted ones, accounted for": the goal's own
/// scope names catalog/page/name-tree traversal as feeding "downstream
/// action, form, annotation, file-specification, colour, font, and content
/// validation" — this function proves each of the other 122 predicates
/// really does land in one of those named families (plus low-level syntax,
/// object limits, and metadata/XMP, which the goal separately excludes from
/// this milestone's scope), with an exhaustive match that panics on any
/// unrecognized local rule id rather than silently miscounting.
fn generated_predicate_family_accounting(
    inventory: &Value,
    catalog_rooted: &BTreeSet<&str>,
) -> Value {
    let predicates = array(&inventory["predicates"], "predicates");
    let mut counts = BTreeMap::<&'static str, usize>::new();
    let mut members = BTreeMap::<&'static str, Vec<String>>::new();
    for predicate in predicates {
        let verapdf_rule_id = string(&predicate["verapdf_rule_id"], "veraPDF rule id");
        let local_checks = array(&predicate["local_checks"], "local checks");
        let first_local_check = string(
            local_checks.first().expect("predicate has a local check"),
            "local check",
        );
        let family = if catalog_rooted.contains(verapdf_rule_id) {
            "document_structure"
        } else {
            predicate_family(first_local_check).expect("local rule has a predicate family")
        };
        *counts.entry(family).or_insert(0) += 1;
        members
            .entry(family)
            .or_default()
            .push(verapdf_rule_id.to_owned());
    }
    let total: usize = counts.values().sum();
    assert_eq!(
        total,
        predicates.len(),
        "predicate family accounting must cover every pinned predicate exactly once"
    );
    assert_eq!(
        counts.get("document_structure").copied().unwrap_or(0),
        catalog_rooted.len(),
        "document_structure family count must match the catalog-rooted matrix"
    );
    json!({
        "total_predicates": total,
        "policy": "Every one of the 129 pinned predicates is assigned to exactly one family below, driven by the local rule id(s) that implement it (see predicate_family in coverage_inventory.rs, an exhaustive match with no wildcard fallback — an unrecognized local rule id fails this test rather than being silently uncounted). document_structure is the 7-predicate catalog-rooted set from the sibling matrix above; every other family name matches the goal's own scope text naming 'downstream action, form, annotation, file-specification, colour, font, and content validation', plus low_level_syntax/object_limits/metadata_and_xmp for the predicates the goal explicitly scopes to other milestones.",
        "family_counts": counts,
        "family_members": members,
    })
}

/// Classifies a local `PDFA1B-*` rule id into its owning family, matching
/// exactly which `validate_document` dispatch function
/// (`crates/page_validation/src/validation.rs`) implements it. Every rule id
/// this crate defines must match one arm; an unmatched id is a bug in this
/// classifier (a new rule was added without updating it), not a shrug —
/// hence the panicking fallback instead of a silent default bucket.
fn predicate_family(local_rule_id: &str) -> Option<&'static str> {
    match local_rule_id {
        // document_structure-eligible ids are intercepted by the caller
        // before this function runs (via the catalog-rooted verapdf id
        // set), except PDFA1B-METADATA-FILTER-001: it shares the exact same
        // catalog Metadata reachability as PDFA1B-METADATA-STRUCTURE-001
        // (which IS catalog-rooted), but is bucketed under metadata_and_xmp
        // here for simplicity since its own object (PDMetadata) is
        // otherwise metadata-only.
        "PDFA1B-METADATA-FILTER-001" => Some("metadata_and_xmp"),

        // annotation: every rule dispatched from validate_annotations.
        "PDFA1B-ANNOTATION-SUBTYPE-001"
        | "PDFA1B-ANNOTATION-OPACITY-001"
        | "PDFA1B-ANNOTATION-FLAGS-001"
        | "PDFA1B-ANNOTATION-COLOR-001"
        | "PDFA1B-ANNOTATION-AP-ENTRIES-001"
        | "PDFA1B-WIDGET-BUTTON-APPEARANCE-001"
        | "PDFA1B-ANNOTATION-NORMAL-APPEARANCE-001" => Some("annotation"),

        // action: every rule dispatched from validate_actions, minus the
        // two already claimed by document_structure (CATALOG-ADDITIONAL-
        // ACTIONS-001, FIELD-ADDITIONAL-ACTIONS-001).
        "PDFA1B-ACTION-TYPE-001"
        | "PDFA1B-NAMED-ACTION-001"
        | "PDFA1B-WIDGET-ACTION-001"
        | "PDFA1B-WIDGET-ADDITIONAL-ACTIONS-001" => Some("action"),

        // form: every rule dispatched from validate_forms, minus
        // ACROFORM-NEED-APPEARANCES-001 (already document_structure).
        "PDFA1B-WIDGET-APPEARANCE-001" => Some("form"),

        // colour: icc_based.rs + device-colour + output-intent checks.
        "PDFA1B-ICCBASED-001"
        | "PDFA1B-ICCBASED-COMPONENTS-001"
        | "PDFA1B-DEVICEN-COMPONENTS-001"
        | "PDFA1B-DEVICE-RGB-001"
        | "PDFA1B-DEVICE-CMYK-001"
        | "PDFA1B-DEVICE-GRAY-001"
        | "PDFA1B-OUTPUTINTENT-001"
        | "PDFA1B-OUTPUTINTENT-IDENTITY-001" => Some("colour"),

        // xobject: every rule dispatched from validate_xobjects.
        "PDFA1B-IMAGE-ALTERNATES-001"
        | "PDFA1B-XOBJECT-OPI-001"
        | "PDFA1B-IMAGE-INTERPOLATE-001"
        | "PDFA1B-IMAGE-BPC-001"
        | "PDFA1B-IMAGE-MASK-BPC-001"
        | "PDFA1B-FORM-POSTSCRIPT-001"
        | "PDFA1B-FORM-REFERENCE-001"
        | "PDFA1B-XOBJECT-POSTSCRIPT-001" => Some("xobject"),

        // graphics: ExtGState/transparency/rendering-intent rules
        // dispatched from validate_graphics (excluding the two below that
        // are bucketed as content, since they concern content-stream
        // execution rather than a graphics-state resource dictionary).
        "PDFA1B-EXTGSTATE-TR-001"
        | "PDFA1B-EXTGSTATE-TR2-001"
        | "PDFA1B-RENDERING-INTENT-001"
        | "PDFA1B-EXTGSTATE-SMASK-001"
        | "PDFA1B-XOBJECT-SMASK-001"
        | "PDFA1B-TRANSPARENCY-GROUP-001"
        | "PDFA1B-EXTGSTATE-BLEND-MODE-001"
        | "PDFA1B-EXTGSTATE-STROKE-ALPHA-001"
        | "PDFA1B-EXTGSTATE-FILL-ALPHA-001" => Some("graphics"),

        // content: facts from the one bounded executor shared by page, Form,
        // annotation-appearance, selected Pattern, and rendered Type3 paths.
        "PDFA1B-CONTENT-OPERATOR-001"
        | "PDFA1B-INLINE-IMAGE-LZW-001"
        | "PDFA1B-GRAPHICS-STATE-NESTING-001" => Some("content"),

        // font: every rule dispatched from validate_font_dictionaries /
        // validate_font_embedding (font_embedding.rs).
        "PDFA1B-FONT-TYPE-001"
        | "PDFA1B-FONT-SUBTYPE-001"
        | "PDFA1B-FONT-BASEFONT-001"
        | "PDFA1B-FONT-FIRSTCHAR-001"
        | "PDFA1B-FONT-LASTCHAR-001"
        | "PDFA1B-FONT-WIDTHS-001"
        | "PDFA1B-FONT-FILE-SUBTYPE-001"
        | "PDFA1B-FONT-EMBEDDING-001"
        | "PDFA1B-TYPE0-CID-SYSTEM-INFO-001"
        | "PDFA1B-CIDTOGIDMAP-001"
        | "PDFA1B-CMAP-EMBEDDING-001"
        | "PDFA1B-CMAP-WMODE-001"
        | "PDFA1B-CMAP-MAX-CID-001"
        | "PDFA1B-TYPE1-SUBSET-CHARSET-001"
        | "PDFA1B-CID-SUBSET-CIDSET-001"
        | "PDFA1B-TRUETYPE-NONSYMBOLIC-ENCODING-001"
        | "PDFA1B-TRUETYPE-NONSYMBOLIC-CMAP-001"
        | "PDFA1B-TRUETYPE-SYMBOLIC-ENCODING-001"
        | "PDFA1B-TRUETYPE-SYMBOLIC-CMAP-001"
        | "PDFA1B-TRUETYPE-GLYPH-PRESENCE-001"
        | "PDFA1B-TYPE1-GLYPH-PRESENCE-001"
        | "PDFA1B-TRUETYPE-GLYPH-WIDTH-001" => Some("font"),

        // metadata_and_xmp: Info dictionary, XMP identification, and XMP
        // extension-schema rules (metadata.rs).
        "PDFA1B-INFO-CREATIONDATE-001"
        | "PDFA1B-INFO-TITLE-001"
        | "PDFA1B-INFO-AUTHOR-001"
        | "PDFA1B-INFO-SUBJECT-001"
        | "PDFA1B-INFO-KEYWORDS-001"
        | "PDFA1B-INFO-CREATOR-001"
        | "PDFA1B-INFO-PRODUCER-001"
        | "PDFA1B-INFO-MODDATE-001"
        | "PDFA1B-ID-SCHEMA-001"
        | "PDFA1B-ID-PART-001"
        | "PDFA1B-ID-CONFORMANCE-001"
        | "PDFA1B-ID-PART-PREFIX-001"
        | "PDFA1B-ID-CONFORMANCE-PREFIX-001"
        | "PDFA1B-ID-AMD-PREFIX-001"
        | "PDFA1B-XMP-PACKET-BYTES-001"
        | "PDFA1B-XMP-PACKET-ENCODING-001"
        | "PDFA1B-XMP-001"
        | "PDFA1B-XMP-PREDEFINED-PROPERTY-001"
        | "PDFA1B-XMP-PREDEFINED-VALUE-TYPE-001"
        | "PDFA1B-XMP-EXTENSION-PROPERTY-DEFINITION-001"
        | "PDFA1B-XMP-EXTENSION-PROPERTY-VALUE-SHAPE-001"
        | "PDFA1B-XMP-EXTENSION-FIELDS-001"
        | "PDFA1B-XMP-EXTENSION-CONTAINER-001"
        | "PDFA1B-XMP-EXTENSION-SCHEMA-NAME-001"
        | "PDFA1B-XMP-EXTENSION-SCHEMA-NAMESPACE-001"
        | "PDFA1B-XMP-EXTENSION-SCHEMA-PREFIX-001"
        | "PDFA1B-XMP-EXTENSION-SCHEMA-PROPERTIES-001"
        | "PDFA1B-XMP-EXTENSION-SCHEMA-VALUE-TYPES-001"
        | "PDFA1B-XMP-EXTENSION-PROPERTY-NAME-001"
        | "PDFA1B-XMP-EXTENSION-PROPERTY-VALUE-TYPE-001"
        | "PDFA1B-XMP-EXTENSION-PROPERTY-CATEGORY-001"
        | "PDFA1B-XMP-EXTENSION-PROPERTY-DESCRIPTION-001"
        | "PDFA1B-XMP-EXTENSION-VALUE-TYPE-NAME-001"
        | "PDFA1B-XMP-EXTENSION-VALUE-TYPE-NAMESPACE-001"
        | "PDFA1B-XMP-EXTENSION-VALUE-TYPE-PREFIX-001"
        | "PDFA1B-XMP-EXTENSION-VALUE-TYPE-DESCRIPTION-001"
        | "PDFA1B-XMP-EXTENSION-VALUE-TYPE-FIELDS-001"
        | "PDFA1B-XMP-EXTENSION-FIELD-NAME-001"
        | "PDFA1B-XMP-EXTENSION-FIELD-VALUE-TYPE-001"
        | "PDFA1B-XMP-EXTENSION-FIELD-DESCRIPTION-001" => Some("metadata_and_xmp"),

        // low_level_syntax: raw file bytes, trailer-direct, and xref/stream
        // structural rules (syntax.rs, stream_safety.rs), independent of
        // any /Root navigation.
        "PDFA1B-HEADER-001"
        | "PDFA1B-HEADER-BINARY-COMMENT-001"
        | "PDFA1B-TRAILER-ID-001"
        | "PDFA1B-ENCRYPTION-001"
        | "PDFA1B-POST-EOF-DATA-001"
        | "PDFA1B-LINEARIZED-TRAILER-ID-001"
        | "PDFA1B-XREF-SUBSECTION-SPACING-001"
        | "PDFA1B-XREF-EOL-001"
        | "PDFA1B-XREF-STREAM-001"
        | "PDFA1B-HEX-STRING-LENGTH-001"
        | "PDFA1B-HEX-STRING-CHARACTERS-001"
        | "PDFA1B-STREAM-LENGTH-001"
        | "PDFA1B-STREAM-EOL-001"
        | "PDFA1B-STREAM-EXTERNAL-DATA-001"
        | "PDFA1B-INDIRECT-OBJECT-SYNTAX-001"
        | "PDFA1B-STREAM-LZW-001" => Some("low_level_syntax"),

        // object_limits: PDF/A-1 §6.1.12 value-range and collection-size
        // limits (object_limits.rs), independent of any /Root navigation.
        "PDFA1B-INTEGER-RANGE-001"
        | "PDFA1B-REAL-RANGE-001"
        | "PDFA1B-STRING-LENGTH-001"
        | "PDFA1B-NAME-LENGTH-001"
        | "PDFA1B-ARRAY-LENGTH-001"
        | "PDFA1B-DICTIONARY-LENGTH-001"
        | "PDFA1B-INDIRECT-OBJECT-COUNT-001" => Some("object_limits"),

        _ => None,
    }
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
        "raw_file" => Some(
            "Evaluate original byte spans and revision-selected syntax; recover only oracle-pinned lexical or xref forms.",
        ),
        "selected_cos_object" => Some(
            "Evaluate the active revision's modeled value using pinned duplicate-key, null, direct/indirect, and name-decoding semantics.",
        ),
        "content_or_embedded_program" => Some(
            "Use the owning bounded content or embedded-program inspector; raw COS provenance is not the source of partial coverage.",
        ),
        _ => None,
    }
    .expect("known provenance class")
}

fn replace_once(bytes: &mut [u8], from: &[u8], to: &[u8]) {
    assert_eq!(from.len(), to.len(), "same-length replacement required");
    let offset = bytes
        .windows(from.len())
        .position(|window| window == from)
        .expect("replacement marker");
    bytes
        .get_mut(offset..offset + to.len())
        .expect("replacement range")
        .copy_from_slice(to);
}

fn replace_once_growing(bytes: &mut Vec<u8>, from: &[u8], to: &[u8]) {
    let offset = bytes
        .windows(from.len())
        .position(|window| window == from)
        .expect("growing replacement marker");
    bytes.splice(offset..offset + from.len(), to.iter().copied());
}

fn write_fixture(fixtures: &Path, name: &str, bytes: &[u8]) {
    fs::write(fixtures.join(name), bytes).expect("write fixture");
}

fn read_json(path: &str) -> Value {
    serde_json::from_slice(&fs::read(path).expect("read JSON fixture")).expect("parse JSON fixture")
}

fn strings(value: &Value, label: &str) -> BTreeSet<String> {
    array(value, label)
        .iter()
        .map(|value| string(value, label).to_owned())
        .collect()
}

fn object<'a>(value: &'a Value, label: &str) -> &'a serde_json::Map<String, Value> {
    value.as_object().expect(label)
}

fn array<'a>(value: &'a Value, label: &str) -> &'a Vec<Value> {
    value.as_array().expect(label)
}

fn string<'a>(value: &'a Value, label: &str) -> &'a str {
    value.as_str().expect(label)
}

fn number(value: &Value, label: &str) -> u64 {
    value.as_u64().expect(label)
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn pdfa_rule_files_cover_every_official_mapping() {
    let inventories = [
        ("pdfa1", 1, "tests/fixtures/pdfa-1b-coverage.json"),
        ("pdfa2", 2, "tests/fixtures/pdfa-2-3-coverage.json"),
        ("pdfa3", 3, "tests/fixtures/pdfa-2-3-coverage.json"),
    ];
    let mut expected_paths = BTreeSet::new();
    for (prefix, part, inventory_path) in inventories {
        let inventory = read_json(inventory_path);
        let mappings = array(&inventory["rule_mapping"]["mappings"], "rule mappings");
        let mut by_reference = BTreeMap::<String, Vec<&Value>>::new();
        for mapping in mappings {
            let reference = string(&mapping["verapdf_rule_id"], "reference rule id");
            let mapping_part = reference
                .strip_prefix("ISO 19005-")
                .and_then(|value| value.chars().next())
                .and_then(|value| value.to_digit(10))
                .expect("PDF/A reference part");
            if mapping_part == part {
                by_reference
                    .entry(reference.to_owned())
                    .or_default()
                    .push(mapping);
            }
        }
        for (reference, mappings) in by_reference {
            let suffix = reference
                .split_once("2005:")
                .or_else(|| reference.split_once("2011:"))
                .or_else(|| reference.split_once("2012:"))
                .map(|(_, suffix)| suffix.replace(['.', ':'], "_"))
                .expect("reference clause");
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join(format!("{prefix}_rule_{suffix}.rs"));
            assert!(
                expected_paths.insert(path.clone()),
                "duplicate rule file claim: {path:?}"
            );
            let source =
                fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
            assert!(
                source.contains("pub mod common;"),
                "{path:?} does not use common helpers"
            );
            assert!(
                source.contains("const REFERENCE_RULE"),
                "{path:?} has no reference rule constant"
            );
            assert!(
                source.contains(&format!("const REFERENCE_RULE: &str = \"{reference}\"")),
                "{path:?} claims the wrong reference rule"
            );
            assert!(source.contains("const CASES"), "{path:?} has no case table");
            assert!(
                source.contains("canonical_pdfa_fixture"),
                "{path:?} has no valid fixture evidence"
            );
            assert!(
                source.contains("fixture_generation"),
                "{path:?} has no ignored fixture-generation test"
            );
            assert!(
                source.contains("verapdf_differential"),
                "{path:?} has no differential test"
            );
            let expected_profiles = mappings
                .iter()
                .flat_map(|mapping| array(&mapping["applicable_profiles"], "applicable profiles"))
                .map(|profile| string(profile, "profile"))
                .collect::<BTreeSet<_>>();
            for profile in expected_profiles {
                let profile_suffix = profile.chars().skip(1).collect::<String>();
                assert!(
                    source.contains(&format!("ReferenceProfile::PdfA{part}{profile_suffix}")),
                    "{path:?} omits applicable profile {profile}"
                );
            }
            for mapping in mappings {
                let local_rule = string(&mapping["canonical_local_rule_id"], "local rule id");
                assert!(
                    source.contains(local_rule),
                    "{path:?} does not assert {local_rule}"
                );
            }
        }
    }

    let actual_paths = fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests"))
        .expect("read integration test directory")
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            (name.starts_with("pdfa1_rule_")
                || name.starts_with("pdfa2_rule_")
                || name.starts_with("pdfa3_rule_"))
            .then_some(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests")
                    .join(name),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual_paths, expected_paths,
        "stale or missing PDF/A rule files"
    );
}
