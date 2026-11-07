pub mod common;

use page_validation::{ValidationOptions, ValidationProfile, validate_pdf_bytes};

const PDFA2_RULE: &str = "ISO 19005-2:2011:6.2.11.4.2:2";
const PDFA1_RULE: &str = "ISO 19005-1:2005:6.3.5:3";

fn failed_rule(bytes: &[u8], profile: ValidationProfile, rule: &str) -> bool {
    validate_pdf_bytes(bytes, &ValidationOptions::default().profile(profile))
        .expect("explicit profile validation")
        .failures
        .iter()
        .any(|failure| failure.rule_id == rule)
}

#[test]
fn pdfa2_checks_unused_embedded_cids_and_nonidentity_mappings() {
    for mapping in ["identity", "nonidentity"] {
        let incomplete = common::pdfa_cidset_subset_fixture(&format!("{mapping}_incomplete"));
        assert!(failed_rule(
            &incomplete,
            ValidationProfile::PdfA2b,
            PDFA2_RULE
        ));

        let complete = common::pdfa_cidset_subset_fixture(&format!("{mapping}_complete"));
        assert!(!failed_rule(
            &complete,
            ValidationProfile::PdfA2b,
            PDFA2_RULE
        ));

        let extra = common::pdfa_cidset_subset_fixture(&format!("{mapping}_extra"));
        assert!(failed_rule(&extra, ValidationProfile::PdfA2b, PDFA2_RULE));
    }
}

#[test]
fn pdfa1_keeps_checking_only_rendered_cids() {
    for mapping in ["identity", "nonidentity"] {
        for case in ["incomplete", "complete", "extra"] {
            let fixture = common::pdfa_cidset_subset_fixture(&format!("{mapping}_{case}"));
            assert!(!failed_rule(
                &fixture,
                ValidationProfile::PdfA1b,
                PDFA1_RULE
            ));
        }
    }
}
