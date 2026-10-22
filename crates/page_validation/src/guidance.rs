//! Provides possible causes and suggested fixes for reported conformance failures.
//!
//! Guidance is selected from the rule identifier and attached to each failure by the validation
//! pipeline. The wording is intentionally brief and advisory because a rule identifier alone
//! cannot determine the exact authoring mistake in a particular PDF.

use serde::{Deserialize, Serialize};

/// A possible explanation and repair suggestion for a validation failure.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct FailureGuidance {
    /// A likely reason the reported requirement was not met.
    pub possible_cause: String,
    /// A suggested way to address the reported issue.
    pub suggested_fix: String,
}

impl FailureGuidance {
    fn from_static(possible_cause: &'static str, suggested_fix: &'static str) -> Self {
        Self {
            possible_cause: possible_cause.to_owned(),
            suggested_fix: suggested_fix.to_owned(),
        }
    }
}

/// Returns guidance for a conformance rule identifier supported by this crate.
pub(crate) fn guidance_for_rule_id(rule_id: &str) -> Option<FailureGuidance> {
    let (standard, suffix) = [
        ("ISO 14289-1:2014:", Standard::PdfUa1),
        ("ISO 19005-1:2005:", Standard::PdfA1),
        ("ISO 19005-2:2011:", Standard::PdfA2Or3),
        ("ISO 19005-3:2012:", Standard::PdfA2Or3),
    ]
    .into_iter()
    .find_map(|(prefix, standard)| {
        rule_id
            .strip_prefix(prefix)
            .map(|suffix| (standard, suffix))
    })?;

    let (clause, _test_number) = suffix.rsplit_once(':')?;
    let (possible_cause, suggested_fix) = match standard {
        Standard::PdfUa1 => pdfua_guidance(clause)?,
        Standard::PdfA1 => pdfa_1_guidance(clause)?,
        Standard::PdfA2Or3 => pdfa_2_or_3_guidance(clause)?,
    };

    Some(FailureGuidance::from_static(possible_cause, suggested_fix))
}

#[derive(Clone, Copy)]
enum Standard {
    PdfUa1,
    PdfA1,
    PdfA2Or3,
}

fn pdfua_guidance(clause: &str) -> Option<(&'static str, &'static str)> {
    Some(match clause {
        "5" => (
            "The PDF/UA identification metadata is absent, malformed, or inconsistent with PDF/UA-1.",
            "Correct the PDF/UA identification properties in the exported XMP metadata.",
        ),
        "6.1" => (
            "The file header does not use a PDF version and line ending accepted by the standard.",
            "Export with a PDF writer that produces a supported PDF header.",
        ),
        "6.2" => (
            "The catalog does not clearly mark the file as a tagged PDF.",
            "Enable tagged PDF output and set MarkInfo/Marked to true.",
        ),
        "7.1" => (
            "The logical structure or handling of decorative content is incomplete or inconsistent.",
            "Use semantic tags for meaningful content and mark decorative content as artifacts.",
        ),
        "7.1.1" => (
            "Decorative content is nested inside tagged content meant to be read.",
            "Move decorative items outside meaningful tags and mark them as artifacts.",
        ),
        "7.1.2" => (
            "Meaningful content is nested inside an artifact.",
            "Move it out of the artifact and assign the appropriate structure tag.",
        ),
        "7.1.3" => (
            "Page content is neither tagged nor marked as an artifact.",
            "Tag meaningful content and mark only decorative graphics or page furniture as artifacts.",
        ),
        "7.1.4" => (
            "The PDF flags its accessibility structure as potentially suspect.",
            "Review the tags and clear the Suspects flag if it is no longer accurate.",
        ),
        "7.1.5" | "7.1.6" | "7.1.7" => (
            "A custom structure type is unmapped, mapped in a loop, or replaces a standard structure type.",
            "Remove invalid role-map entries; map custom tags to standard types without redefining standard tags.",
        ),
        "7.1.8" | "7.1.9" | "7.1.10" => (
            "Required metadata or the title-display preference is missing or invalid.",
            "Add valid XMP metadata with the document title and enable title display in viewer preferences.",
        ),
        "7.1.11" | "7.1.12" => (
            "The structure tree or a structure element's parent link is incomplete.",
            "Repair the structure tree and parent links in the PDF authoring tool.",
        ),
        "7.2" => (
            "A table, list, bookmark, annotation, form label, or language property has invalid structure.",
            "Use valid semantic nesting and set language where required.",
        ),
        _ if clause.starts_with("7.2.") => (
            "A table, list, bookmark, annotation, form label, or language property has invalid structure.",
            "Use valid semantic nesting and set language where required.",
        ),
        "7.3" => (
            "A figure lacks a usable text alternative.",
            "Add concise alternative text to informative figures; mark decorative figures as artifacts.",
        ),
        "7.4.2" | "7.4.4" => (
            "Headings skip levels, repeat at a level, or mix hierarchies.",
            "Use one consistent hierarchy with ordered levels and clear section headings.",
        ),
        "7.5" => (
            "Table headers are missing or unassociated with their data.",
            "Add header cells and associate them with the rows or columns they describe.",
        ),
        "7.7" => (
            "A formula lacks a description for nonvisual reading.",
            "Tag formulas and provide replacement text conveying their meaning or spoken form.",
        ),
        "7.9" => (
            "A note lacks a unique identifier or shares one with another note.",
            "Assign unique identifiers and link each reference to its note in the structure tree.",
        ),
        "7.10" => (
            "An optional-content group is unnamed or uses a prohibited visibility setting.",
            "Name each group and remove settings that change its visibility automatically.",
        ),
        "7.11" => (
            "An attachment is missing a required filename value.",
            "Set non-empty filenames in both the F and UF entries.",
        ),
        "7.15" => (
            "The PDF contains a dynamic XFA form, which PDF/UA-1 does not support.",
            "Replace XFA with a tagged form or an accessible non-interactive document.",
        ),
        "7.16" => (
            "Encryption prevents assistive technology from extracting document content.",
            "Allow accessibility and content extraction in the security settings, or remove encryption.",
        ),
        _ if clause.starts_with("7.18.") => (
            "An annotation, link, media item, or form widget is missing required structure or descriptive data.",
            "Tag each item, add an accessible description, set page tab order, and remove unsupported annotation types.",
        ),
        "7.20" => (
            "A form or reference XObject is missing from the tagged structure or represented incorrectly.",
            "Associate reusable graphics with structure elements and embed referenced content.",
        ),
        "7.20.1" => (
            "A reference XObject points to content not embedded in the PDF.",
            "Embed the referenced graphic or remove the external reference.",
        ),
        "7.20.2" => (
            "A reusable form graphic is not connected to the structure at each use.",
            "Represent each use in the structure tree and preserve reading order.",
        ),
        _ if clause.starts_with("7.21.") => (
            "A font program, encoding, glyph inventory, or Unicode map is incomplete or inconsistent.",
            "Embed fonts with valid glyph and width data and complete Unicode maps.",
        ),
        _ => return None,
    })
}

fn pdfa_1_guidance(clause: &str) -> Option<(&'static str, &'static str)> {
    let mut parts = clause.split('.');
    if parts.next() != Some("6") {
        return None;
    }

    Some(match parts.next()? {
        "1" => (
            "The PDF header, object syntax, trailer, or cross-reference data is missing or malformed for PDF/A.",
            "Repair the file structure or regenerate with a PDF/A writer; remove encryption.",
        ),
        "2" => (
            "A page graphic uses a color space, image feature, or output intent that PDF/A-1 does not permit.",
            "Convert graphics to permitted color spaces and add a valid output intent.",
        ),
        "3" => (
            "A font program, descriptor, encoding, or character map is missing or invalid.",
            "Embed fonts with valid glyph and encoding data.",
        ),
        "4" => (
            "The document uses transparency in a graphics state, which PDF/A-1 does not allow.",
            "Flatten transparency before export or use a profile that permits it.",
        ),
        "5" => (
            "An annotation type or property is not permitted by PDF/A-1.",
            "Remove or flatten unsupported annotations; correct remaining flags, opacity, and appearances.",
        ),
        "6" => (
            "The document contains an action that PDF/A-1 prohibits.",
            "Remove the action or replace it with static content.",
        ),
        "7" => (
            "XMP is missing, malformed, or inconsistent with the Info dictionary.",
            "Correct required XMP properties and synchronize matching Info values.",
        ),
        "8" => (
            "The tagged structure, role mapping, or language information is incomplete or invalid.",
            "Repair the structure tree, map custom tags to standard types, and set the document language.",
        ),
        "9" => (
            "The interactive form dictionary contains a setting PDF/A-1 does not permit.",
            "Disable automatic appearance generation and provide an appearance stream for each field.",
        ),
        _ => return None,
    })
}

fn pdfa_2_or_3_guidance(clause: &str) -> Option<(&'static str, &'static str)> {
    let mut parts = clause.split('.');
    if parts.next() != Some("6") {
        return None;
    }

    Some(match parts.next()? {
        "1" => (
            "The PDF header, object syntax, trailer, or cross-reference data is missing or malformed for PDF/A.",
            "Repair the file structure or regenerate with a PDF/A writer; remove encryption.",
        ),
        "2" => (
            "A graphic, color profile, image, or font uses data the selected PDF/A profile forbids.",
            "Correct the object and use supported color, font, image, and graphics data.",
        ),
        "3" => (
            "An annotation's type, flag, appearance, or description violates PDF/A.",
            "Remove or flatten unsupported annotations; repair remaining flags, appearances, and descriptions.",
        ),
        "4" => (
            "An interactive form or signature dictionary contains a prohibited key or inconsistent signature data.",
            "Remove prohibited keys and repair signatures with a PDF/A-compatible workflow.",
        ),
        "5" => (
            "The document contains an action or additional-action entry that the selected PDF/A profile forbids.",
            "Remove the prohibited action; replace it with static content if needed.",
        ),
        "6" => (
            "XMP is missing, malformed, or inconsistent with the Info dictionary.",
            "Correct required XMP properties and extension schemas; synchronize matching Info values.",
        ),
        "7" => (
            "The tagged structure, role mapping, or language information is incomplete or invalid.",
            "Repair the structure tree, map custom tags to standard types, and set the document language.",
        ),
        "8" => (
            "An embedded file lacks metadata or file-specification entries required by the selected PDF/A part.",
            "Add the required filename and MIME type, and associate the attachment as required.",
        ),
        "9" => (
            "An optional-content group or configuration has a name or setting disallowed by PDF/A.",
            "Name each layer and remove prohibited visibility settings.",
        ),
        "10" => (
            "The document contains an alternate presentation entry not permitted by the selected PDF/A profile.",
            "Remove the alternate presentation and retain static page content.",
        ),
        "11" => (
            "The document catalog contains a requirements entry that the selected PDF/A profile forbids.",
            "Remove the Requirements entry from the catalog.",
        ),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::guidance_for_rule_id;

    #[test]
    fn provides_guidance_for_pdfua_and_pdfa_failures() {
        for rule_id in [
            "ISO 14289-1:2014:5:1",
            "ISO 14289-1:2014:7.2:29",
            "ISO 14289-1:2014:7.21.7.2:1",
            "ISO 19005-1:2005:6.7.3:2",
            "ISO 19005-2:2011:6.6.2.3.3:18",
            "ISO 19005-3:2012:6.8:1",
        ] {
            let guidance = guidance_for_rule_id(rule_id).expect("known rule guidance");
            assert!(!guidance.possible_cause.is_empty());
            assert!(!guidance.suggested_fix.is_empty());
        }
    }

    #[test]
    fn covers_every_implemented_rule_clause() {
        let rule_clauses: &[(&str, &[&str])] = &[
            (
                "ISO 14289-1:2014:",
                &[
                    "5", "6.1", "6.2", "7.1", "7.2", "7.3", "7.4.2", "7.4.4", "7.5", "7.7", "7.9",
                    "7.10", "7.11", "7.15", "7.16", "7.18.1", "7.18.2", "7.18.3", "7.18.4",
                    "7.18.5", "7.18.6.2", "7.18.8", "7.20", "7.21.3.1", "7.21.3.2", "7.21.3.3",
                    "7.21.4.1", "7.21.4.2", "7.21.5", "7.21.6", "7.21.7", "7.21.8",
                ],
            ),
            (
                "ISO 19005-1:2005:",
                &[
                    "6.1.2", "6.1.3", "6.1.4", "6.2.2", "6.7.2", "6.7.3", "6.7.9", "6.7.11",
                ],
            ),
            (
                "ISO 19005-2:2011:",
                &["6.1.7.2", "6.1.10", "6.4.1", "6.6.2.1", "6.6.4"],
            ),
            (
                "ISO 19005-3:2012:",
                &["6.1.7.2", "6.1.10", "6.4.1", "6.6.2.1", "6.6.4"],
            ),
        ];

        for (prefix, clauses) in rule_clauses {
            for clause in *clauses {
                let rule_id = format!("{prefix}{clause}:1");
                assert!(
                    guidance_for_rule_id(&rule_id).is_some(),
                    "missing guidance for {rule_id}",
                );
            }
        }
    }

    #[test]
    fn leaves_unknown_rule_identifiers_without_guidance() {
        assert!(guidance_for_rule_id("RULE-UNKNOWN").is_none());
        assert!(guidance_for_rule_id("ISO 19005-4:2020:6.1:1").is_none());
        assert!(guidance_for_rule_id("ISO 14289-1:2014:7.22:1").is_none());
    }
}
