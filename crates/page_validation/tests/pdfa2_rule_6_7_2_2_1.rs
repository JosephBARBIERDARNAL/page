use page_validation::differential::ReferenceProfile;

pub mod common;

const RULE: &str = "PDFA1A-TAGGED-DOCUMENT-001";
const ADDITIONAL_RULES: &[&str] = &[];
const REFERENCE_RULE: &str = "ISO 19005-2:2011:6.7.2.2:1";
const PROFILES: &[ReferenceProfile] = &[ReferenceProfile::PdfA2a];

crate::pdfa_rule_tests! {
    rule: RULE,
    additional_rules: ADDITIONAL_RULES,
    reference_rule: REFERENCE_RULE,
    profiles: PROFILES,
    fixture_stem: "pdfa2-rule-6-7-2-2-1",
    label: "maintenance generator for PDFA2 rule 6.7.2.2-1 fixtures",
    include_invalid: false,
}
