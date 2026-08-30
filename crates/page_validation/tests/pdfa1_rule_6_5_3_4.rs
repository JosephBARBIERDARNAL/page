use page_validation::differential::ReferenceProfile;

pub mod common;

const RULE: &str = "PDFA1B-ANNOTATION-AP-ENTRIES-001";
const ADDITIONAL_RULES: &[&str] = &[];
const REFERENCE_RULE: &str = "ISO 19005-1:2005:6.5.3:4";
const PROFILES: &[ReferenceProfile] = &[ReferenceProfile::PdfA1a, ReferenceProfile::PdfA1b];

crate::pdfa_rule_tests! {
    rule: RULE,
    additional_rules: ADDITIONAL_RULES,
    reference_rule: REFERENCE_RULE,
    profiles: PROFILES,
    fixture_stem: "pdfa1-rule-6-5-3-4",
    label: "maintenance generator for PDFA1 rule 6.5.3-4 fixtures",
    include_invalid: true,
}
