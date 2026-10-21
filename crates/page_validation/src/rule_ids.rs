//! Maps internal validation checks to the rule identifiers used by veraPDF.
//!
//! The mapping is keyed by profile and internal check ID. Reports expose only the corresponding
//! veraPDF identifier; parser and operational errors remain classified separately as `PageError`.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::validation::ValidationProfile;

const RULE_ID_MAPPINGS: &str = include_str!("rule_ids.tsv");

pub(crate) fn verapdf_rule_id(
    profile: ValidationProfile,
    internal_rule_id: &str,
) -> Option<String> {
    static MAPPINGS: OnceLock<HashMap<&'static str, HashMap<&'static str, &'static str>>> =
        OnceLock::new();

    let mappings = MAPPINGS.get_or_init(|| {
        let mut profiles = HashMap::<&'static str, HashMap<&'static str, &'static str>>::new();
        for line in RULE_ID_MAPPINGS
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
        {
            let mut fields = line.split('\t');
            let (Some(profile), Some(internal_rule_id), Some(verapdf_rule_id), None) =
                (fields.next(), fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            profiles
                .entry(profile)
                .or_default()
                .insert(internal_rule_id, verapdf_rule_id);
        }
        profiles
    });

    if let Some(rule_id) = mappings
        .get(profile.as_str())
        .and_then(|profile_mappings| profile_mappings.get(internal_rule_id))
    {
        return Some((*rule_id).to_owned());
    }

    let part = profile.pdfa_part()?;
    let suffix = internal_rule_id.split_once('-')?.1;
    let base_profiles: &[(&str, u8, char)] = match (part, profile.pdfa_conformance()) {
        (1, Some('A')) => &[("1b", 1, 'B')],
        (2, Some('A' | 'U')) => &[("2b", 2, 'B')],
        (3, Some('A')) => &[("2a", 2, 'A'), ("3b", 3, 'B'), ("2b", 2, 'B')],
        (3, Some('B')) => &[("2b", 2, 'B')],
        (3, Some('U')) => &[("2u", 2, 'U'), ("3b", 3, 'B'), ("2b", 2, 'B')],
        _ => return None,
    };
    for &(base_profile, base_part, base_level) in base_profiles {
        let base_rule_id = format!("PDFA{base_part}{base_level}-{suffix}");
        let Some(rule_id) = mappings
            .get(base_profile)
            .and_then(|profile_mappings| profile_mappings.get(base_rule_id.as_str()))
        else {
            continue;
        };

        return Some(if part == 3 && base_part == 2 {
            rule_id.replace("ISO 19005-2:2011:", "ISO 19005-3:2012:")
        } else {
            (*rule_id).to_owned()
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::verapdf_rule_id;
    use crate::validation::ValidationProfile;

    #[test]
    fn maps_pdfa2b_corpus_findings_and_pdfa3_fallbacks() {
        for (local_rule_id, vera_rule_id) in [
            (
                "PDFA2B-CATALOG-ADDITIONAL-ACTIONS-001",
                "ISO 19005-2:2011:6.5.2:1",
            ),
            (
                "PDFA2B-CATALOG-NEEDS-RENDERING-001",
                "ISO 19005-2:2011:6.4.2:2",
            ),
            ("PDFA2B-CMAP-CID-RANGE-001", "ISO 19005-2:2011:6.1.13:10"),
            ("PDFA2B-CMAP-EMBEDDING-001", "ISO 19005-2:2011:6.2.11.3.3:1"),
            (
                "PDFA2B-FIELD-ADDITIONAL-ACTIONS-001",
                "ISO 19005-2:2011:6.4.1:2",
            ),
            (
                "PDFA2B-ID-CONFORMANCE-PREFIX-001",
                "ISO 19005-2:2011:6.6.4:5",
            ),
            ("PDFA2B-ID-PART-PREFIX-001", "ISO 19005-2:2011:6.6.4:4"),
            ("PDFA2B-ID-SCHEMA-001", "ISO 19005-2:2011:6.6.4:1"),
            (
                "PDFA2B-JPEG2000-COLOR-SPECS-001",
                "ISO 19005-2:2011:6.2.8.3:2",
            ),
            ("PDFA2B-REAL-MINIMUM-001", "ISO 19005-2:2011:6.1.13:5"),
            (
                "PDFA2B-TRUETYPE-GLYPH-PRESENCE-001",
                "ISO 19005-2:2011:6.2.11.4.1:2",
            ),
            (
                "PDFA2B-TYPE1-GLYPH-PRESENCE-001",
                "ISO 19005-2:2011:6.2.11.4.1:2",
            ),
            ("PDFA2B-WIDGET-APPEARANCE-001", "ISO 19005-2:2011:6.3.3:1"),
            (
                "PDFA2B-XMP-EXTENSION-FIELD-NAME-001",
                "ISO 19005-2:2011:6.6.2.3.3:16",
            ),
            (
                "PDFA2B-XMP-EXTENSION-FIELD-VALUE-TYPE-001",
                "ISO 19005-2:2011:6.6.2.3.3:17",
            ),
            (
                "PDFA2B-XMP-EXTENSION-PROPERTY-DEFINITION-001",
                "ISO 19005-2:2011:6.6.2.3.1:1",
            ),
            (
                "PDFA2B-XMP-EXTENSION-PROPERTY-NAME-001",
                "ISO 19005-2:2011:6.6.2.3.3:7",
            ),
            (
                "PDFA2B-XMP-EXTENSION-PROPERTY-VALUE-SHAPE-001",
                "ISO 19005-2:2011:6.6.2.3.1:2",
            ),
            (
                "PDFA2B-XMP-EXTENSION-PROPERTY-VALUE-TYPE-001",
                "ISO 19005-2:2011:6.6.2.3.3:8",
            ),
            (
                "PDFA2B-XMP-EXTENSION-SCHEMA-NAMESPACE-001",
                "ISO 19005-2:2011:6.6.2.3.3:3",
            ),
            (
                "PDFA2B-XMP-EXTENSION-SCHEMA-PROPERTIES-001",
                "ISO 19005-2:2011:6.6.2.3.3:5",
            ),
            (
                "PDFA2B-XMP-EXTENSION-SCHEMA-VALUE-TYPES-001",
                "ISO 19005-2:2011:6.6.2.3.3:6",
            ),
            (
                "PDFA2B-XMP-EXTENSION-VALUE-TYPE-FIELDS-001",
                "ISO 19005-2:2011:6.6.2.3.3:15",
            ),
            (
                "PDFA2B-XMP-EXTENSION-VALUE-TYPE-NAME-001",
                "ISO 19005-2:2011:6.6.2.3.3:11",
            ),
            (
                "PDFA2B-XMP-EXTENSION-VALUE-TYPE-NAMESPACE-001",
                "ISO 19005-2:2011:6.6.2.3.3:12",
            ),
            (
                "PDFA2B-XMP-PREDEFINED-VALUE-TYPE-001",
                "ISO 19005-2:2011:6.6.2.3.1:2",
            ),
            (
                "PDFA2B-XMP-PROPERTY-DEFINITION-001",
                "ISO 19005-2:2011:6.6.2.3.1:1",
            ),
        ] {
            assert_eq!(
                verapdf_rule_id(ValidationProfile::PdfA2b, local_rule_id).as_deref(),
                Some(vera_rule_id),
                "{local_rule_id}"
            );
        }
        assert_eq!(
            verapdf_rule_id(ValidationProfile::PdfA3b, "PDFA3B-CMAP-CID-RANGE-001").as_deref(),
            Some("ISO 19005-3:2012:6.1.13:10")
        );
    }
}
