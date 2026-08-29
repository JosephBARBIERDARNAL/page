#!/usr/bin/env python3
"""Generate one integration test per mapped PDF/A rule."""

from collections import defaultdict
from pathlib import Path
import json
import re


ROOT = Path(__file__).resolve().parents[1]
TESTS = ROOT / "crates/page_validation/tests"
FIXTURES = TESTS / "fixtures"
MUTATION_LOCAL_RULES = {
    mutation["local_rule_id"]
    for mutation in json.loads(
        (FIXTURES / "verapdf-diff-cases.json").read_text()
    )["checked_in_mutations"]
}


def reference_parts(reference: str):
    match = re.fullmatch(r"ISO 19005-(\d):\d{4}:([0-9.]+):(\d+)", reference)
    if not match:
        raise ValueError(f"unsupported reference rule: {reference}")
    return int(match.group(1)), match.group(2), int(match.group(3))


def rust_profile(profile: str) -> str:
    return f"ReferenceProfile::PdfA{profile[0]}{profile[1]}"


def rule_file(part: int, clause: str, test_number: int) -> Path:
    return TESTS / f"pdfa{part}_rule_{clause.replace('.', '_')}_{test_number}.rs"


def render(prefix: str, reference: str, mappings: list[dict]) -> str:
    part, clause, test_number = reference_parts(reference)
    local_rules = sorted({mapping["canonical_local_rule_id"] for mapping in mappings})
    profiles = sorted(
        {profile for mapping in mappings for profile in mapping["applicable_profiles"]}
    )
    primary, *additional = local_rules
    profiles_rust = ", ".join(rust_profile(profile) for profile in profiles)
    additional_rust = ", ".join(f'"{rule}"' for rule in additional)
    has_mutation = prefix == "pdfa1" and any(
        rule in MUTATION_LOCAL_RULES for rule in local_rules
    )
    stem = f"{prefix}_rule_{clause.replace('.', '_')}_{test_number}"
    fixture_stem = f"pdfa{part}-rule-{clause.replace('.', '-')}-{test_number}"
    cases = '[ ("valid", false), ("invalid", true) ]' if has_mutation else '[ ("valid", false) ]'
    return f'''use page_validation::differential::ReferenceProfile;

pub mod common;

const RULE: &str = "{primary}";
const ADDITIONAL_RULES: &[&str] = &[{additional_rust}];
const REFERENCE_RULE: &str = "{reference}";
const PROFILES: &[ReferenceProfile] = &[{profiles_rust}];
const CASES: &[(&str, bool)] = &{cases};

#[test]
fn {stem}_local_validation() {{
    let mut local_rules = vec![RULE];
    local_rules.extend_from_slice(ADDITIONAL_RULES);
    for (case, should_fail) in CASES {{
        match *case {{
            "valid" => for profile in PROFILES {{
                let bytes = common::pdfa_profile_fixture(
                    *profile,
                    common::canonical_pdfa_fixture(*profile),
                );
                common::assert_pdfa_rule_behavior(*profile, &local_rules, &bytes, *should_fail);
            }},
            "invalid" => for local_rule in &local_rules {{
                if let Some(source) = common::mutation_fixture(local_rule) {{
                    let profile = common::preferred_pdfa_mutation_profile(&[*local_rule], PROFILES);
                    let bytes = common::pdfa_profile_fixture(profile, &source);
                    common::assert_pdfa_rule_behavior(profile, &local_rules, &bytes, *should_fail);
                    break;
                }}
            }},
            _ => panic!("unknown PDF/A rule case: {{case}}"),
        }}
    }}
}}

#[test]
#[ignore = "maintenance generator for {prefix.upper()} rule {clause}-{test_number} fixtures"]
fn {stem}_fixture_generation() {{
    let profile = PROFILES[0];
    let bytes = common::pdfa_profile_fixture(profile, common::canonical_pdfa_fixture(profile));
    common::write_generated_fixture("{fixture_stem}-valid.pdf", &bytes);
    let mut local_rules = vec![RULE];
    local_rules.extend_from_slice(ADDITIONAL_RULES);
    for local_rule in &local_rules {{
        if let Some(source) = common::mutation_fixture(local_rule) {{
            common::write_generated_fixture("{fixture_stem}-invalid.pdf", &source);
            break;
        }}
    }}
}}

#[test]
fn {stem}_verapdf_differential_when_opted_in() {{
    let mut local_rules = vec![RULE];
    local_rules.extend_from_slice(ADDITIONAL_RULES);
    common::run_pdfa_rule_differential(REFERENCE_RULE, &local_rules, PROFILES);
}}
'''


def main():
    inventories = [
        ("pdfa1", json.loads((FIXTURES / "pdfa-1b-coverage.json").read_text())["rule_mapping"]["mappings"]),
        ("pdfa2", json.loads((FIXTURES / "pdfa-2-3-coverage.json").read_text())["rule_mapping"]["mappings"]),
        ("pdfa3", json.loads((FIXTURES / "pdfa-2-3-coverage.json").read_text())["rule_mapping"]["mappings"]),
    ]
    grouped = defaultdict(list)
    for prefix, mappings in inventories:
        for mapping in mappings:
            part, _, _ = reference_parts(mapping["verapdf_rule_id"])
            if (prefix == "pdfa1" and part == 1) or (prefix == f"pdfa{part}" and part in (2, 3)):
                grouped[(prefix, mapping["verapdf_rule_id"])].append(mapping)

    expected = set()
    for (prefix, reference), mappings in sorted(grouped.items()):
        part, clause, test_number = reference_parts(reference)
        path = rule_file(part, clause, test_number)
        expected.add(path)
        path.write_text(render(prefix, reference, mappings))

    for path in TESTS.glob("pdfa[123]_rule_*.rs"):
        if path not in expected:
            path.unlink()
    print(f"generated {len(expected)} PDF/A rule files")


if __name__ == "__main__":
    main()
