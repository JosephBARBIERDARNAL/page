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
