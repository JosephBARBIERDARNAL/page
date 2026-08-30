use page_validation::differential::ReferenceProfile;

pub mod common;

const RULE: &str = "PDFA1B-XMP-EXTENSION-PROPERTY-VALUE-SHAPE-001";
const ADDITIONAL_RULES: &[&str] = &["PDFA1B-XMP-PREDEFINED-VALUE-TYPE-001"];
const REFERENCE_RULE: &str = "ISO 19005-1:2005:6.7.9:3";
const PROFILES: &[ReferenceProfile] = &[ReferenceProfile::PdfA1a, ReferenceProfile::PdfA1b];

crate::pdfa_rule_tests! {
    rule: RULE,
    additional_rules: ADDITIONAL_RULES,
    reference_rule: REFERENCE_RULE,
    profiles: PROFILES,
    fixture_stem: "pdfa1-rule-6-7-9-3",
    label: "maintenance generator for PDFA1 rule 6.7.9-3 fixtures",
    include_invalid: true,
}
