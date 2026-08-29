use page_validation::differential::ReferenceProfile;

pub mod common;

const RULE: &str = "PDFA1B-EXTGSTATE-STROKE-ALPHA-001";
const ADDITIONAL_RULES: &[&str] = &[];
const REFERENCE_RULE: &str = "ISO 19005-1:2005:6.4:5";
const PROFILES: &[ReferenceProfile] = &[ReferenceProfile::PdfA1a, ReferenceProfile::PdfA1b];
const CASES: &[(&str, bool)] = &[("valid", false), ("invalid", true)];

#[test]
fn pdfa1_rule_6_4_5_local_validation() {
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
#[ignore = "maintenance generator for PDFA1 rule 6.4-5 fixtures"]
fn pdfa1_rule_6_4_5_fixture_generation() {
    let profile = PROFILES[0];
    let bytes = common::pdfa_profile_fixture(profile, common::canonical_pdfa_fixture(profile));
    common::write_generated_fixture("pdfa1-rule-6-4-5-valid.pdf", &bytes);
    let mut local_rules = vec![RULE];
    local_rules.extend_from_slice(ADDITIONAL_RULES);
    for local_rule in &local_rules {
        if let Some(source) = common::mutation_fixture(local_rule) {
            common::write_generated_fixture("pdfa1-rule-6-4-5-invalid.pdf", &source);
            break;
        }
    }
}

#[test]
fn pdfa1_rule_6_4_5_verapdf_differential_when_opted_in() {
    let mut local_rules = vec![RULE];
    local_rules.extend_from_slice(ADDITIONAL_RULES);
    common::run_pdfa_rule_differential(REFERENCE_RULE, &local_rules, PROFILES);
}
