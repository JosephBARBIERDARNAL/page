use page_validation::differential::ReferenceProfile;

pub mod common;

const RULE: &str = "PDFA1B-FILE-SPEC-ASSOCIATION-001";
const ADDITIONAL_RULES: &[&str] = &[];
const REFERENCE_RULE: &str = "ISO 19005-3:2012:6.8:4";
const PROFILES: &[ReferenceProfile] = &[
    ReferenceProfile::PdfA3a,
    ReferenceProfile::PdfA3b,
    ReferenceProfile::PdfA3u,
];

crate::pdfa_rule_tests! {
    rule: RULE,
    additional_rules: ADDITIONAL_RULES,
    reference_rule: REFERENCE_RULE,
    profiles: PROFILES,
    fixture_stem: "pdfa3-rule-6-8-4",
    label: "maintenance generator for PDFA3 rule 6.8-4 fixtures",
    include_invalid: false,
}
