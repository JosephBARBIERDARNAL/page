use page_validation::differential::ReferenceProfile;

pub mod common;

const RULE: &str = "PDFA1B-SEPARATION-CONSISTENCY-001";
const ADDITIONAL_RULES: &[&str] = &[];
const REFERENCE_RULE: &str = "ISO 19005-3:2012:6.2.4.4:2";
const PROFILES: &[ReferenceProfile] = &[
    ReferenceProfile::PdfA3a,
    ReferenceProfile::PdfA3b,
    ReferenceProfile::PdfA3u,
];
const CASES: &[(&str, bool)] = &[("valid", false)];

#[test]
fn pdfa3_rule_6_2_4_4_2_local_validation() {
    let mut local_rules = vec![RULE];
    local_rules.extend_from_slice(ADDITIONAL_RULES);
    for (case, should_fail) in CASES {
        match *case {
            "valid" => {
                for profile in PROFILES {
                    let bytes = common::pdfa_profile_fixture(
                        *profile,
                        common::canonical_pdfa_fixture(*profile),
                    );
                    common::assert_pdfa_rule_behavior(*profile, &local_rules, &bytes, *should_fail);
                }
            }
            "invalid" => {
                for local_rule in &local_rules {
                    if let Some(source) = common::mutation_fixture(local_rule) {
                        let profile =
                            common::preferred_pdfa_mutation_profile(&[*local_rule], PROFILES);
                        let bytes = common::pdfa_profile_fixture(profile, &source);
                        common::assert_pdfa_rule_behavior(
                            profile,
                            &local_rules,
                            &bytes,
                            *should_fail,
                        );
                        break;
                    }
                }
            }
            _ => panic!("unknown PDF/A rule case: {case}"),
        }
    }
}

#[test]
#[ignore = "maintenance generator for PDFA3 rule 6.2.4.4-2 fixtures"]
fn pdfa3_rule_6_2_4_4_2_fixture_generation() {
    let profile = PROFILES[0];
    let bytes = common::pdfa_profile_fixture(profile, common::canonical_pdfa_fixture(profile));
    common::write_generated_fixture("pdfa3-rule-6-2-4-4-2-valid.pdf", &bytes);
    let mut local_rules = vec![RULE];
    local_rules.extend_from_slice(ADDITIONAL_RULES);
    for local_rule in &local_rules {
        if let Some(source) = common::mutation_fixture(local_rule) {
            common::write_generated_fixture("pdfa3-rule-6-2-4-4-2-invalid.pdf", &source);
            break;
        }
    }
}

#[test]
fn pdfa3_rule_6_2_4_4_2_verapdf_differential_when_opted_in() {
    let mut local_rules = vec![RULE];
    local_rules.extend_from_slice(ADDITIONAL_RULES);
    common::run_pdfa_rule_differential(REFERENCE_RULE, &local_rules, PROFILES);
}
