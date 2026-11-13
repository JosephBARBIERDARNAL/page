use std::collections::BTreeSet;

use page_validation::{ValidationOptions, ValidationProfile, validate_pdf_bytes};

pub mod common;

#[test]
fn utf16_cmap_is_unavailable_for_pdfa1_compatibility() {
    for encoding in ["UniKS-UTF16-H", "UniKS-UTF16-V"] {
        assert_named_cmap_rules(encoding, "Korea1", true, false);
    }
}

#[test]
fn ucs2_cmap_retains_pdfa1_compatibility_and_embedding_failure() {
    for encoding in ["UniKS-UCS2-H", "UniKS-UCS2-V"] {
        assert_named_cmap_rules(encoding, "Korea1", false, false);
    }
}

#[test]
fn identity_cmaps_are_exempt_from_compatibility_and_embedding() {
    for encoding in ["Identity-H", "Identity-V"] {
        assert_named_cmap_rules(encoding, "Different", false, false);
    }
}

#[test]
fn wrong_cid_ordering_fails_compatibility_for_every_profile() {
    assert_named_cmap_rules("UniKS-UCS2-H", "Different", true, true);
}

fn assert_named_cmap_rules(
    encoding: &str,
    ordering: &str,
    pdfa1_mismatch: bool,
    later_mismatch: bool,
) {
    let bytes = common::unembedded_type0_cmap_fixture(encoding, ordering);
    for (profile, base_rules, compatibility_rule) in [
        (
            ValidationProfile::PdfA1b,
            vec![
                "ISO 19005-1:2005:6.2.3.3:3",
                "ISO 19005-1:2005:6.3.4:1",
                "ISO 19005-1:2005:6.7.2:1",
            ],
            "ISO 19005-1:2005:6.3.3.1:1",
        ),
        (
            ValidationProfile::PdfA2b,
            vec![
                "ISO 19005-2:2011:6.2.11.4.1:1",
                "ISO 19005-2:2011:6.2.4.3:4",
                "ISO 19005-2:2011:6.6.2.1:1",
            ],
            "ISO 19005-2:2011:6.2.11.3.1:1",
        ),
        (
            ValidationProfile::PdfA3b,
            vec![
                "ISO 19005-3:2012:6.2.11.4.1:1",
                "ISO 19005-3:2012:6.2.4.3:4",
                "ISO 19005-3:2012:6.6.2.1:1",
            ],
            "ISO 19005-3:2012:6.2.11.3.1:1",
        ),
        (
            ValidationProfile::PdfUa1,
            vec![
                "ISO 14289-1:2014:6.2:1",
                "ISO 14289-1:2014:7.1:10",
                "ISO 14289-1:2014:7.1:11",
                "ISO 14289-1:2014:7.1:3",
                "ISO 14289-1:2014:7.1:8",
                "ISO 14289-1:2014:7.21.4.1:1",
                "ISO 14289-1:2014:7.2:34",
            ],
            "ISO 14289-1:2014:7.21.3.1:1",
        ),
    ] {
        let mut expected = base_rules.into_iter().collect::<BTreeSet<_>>();
        let pdfa1 = profile == ValidationProfile::PdfA1b;
        if pdfa1 && !encoding.starts_with("Identity-") {
            expected.insert("ISO 19005-1:2005:6.3.3.3:1");
        }
        if profile == ValidationProfile::PdfUa1 && ordering == "Different" {
            // Without a recognized collection or ToUnicode, Unicode mapping
            // also fails independently of CIDSystemInfo compatibility.
            expected.insert("ISO 14289-1:2014:7.21.7:1");
        }
        if if pdfa1 {
            pdfa1_mismatch
        } else {
            later_mismatch
        } {
            expected.insert(compatibility_rule);
        }
        let report = validate_pdf_bytes(&bytes, &ValidationOptions::default().profile(profile))
            .expect("validate named CMap fixture");
        let actual = report
            .failures
            .iter()
            .map(|failure| failure.rule_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected, "{profile}: {encoding}, {ordering}");
    }
}
