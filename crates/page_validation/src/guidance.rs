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
            "Correct the PDF/UA identification properties in the exported XMP metadata, then validate the regenerated PDF.",
        ),
        "6.1" => (
            "The file header does not use a PDF version and line ending accepted by the standard.",
            "Export with a PDF writer that produces a supported PDF header and avoid editing the header by hand.",
        ),
        "6.2" => (
            "The catalog does not clearly mark the file as a tagged PDF.",
            "Enable tagged PDF output so the document contains the required MarkInfo entry with Marked set to true.",
        ),
        "7.1" => (
            "The document's logical structure or treatment of decorative content is incomplete or inconsistent.",
            "Review the tag tree, represent meaningful content with semantic tags, and mark decorative content as artifacts.",
        ),
        "7.1.1" => (
            "Decorative content is nested inside content that is meant to be read as real document content.",
            "Mark decorative items as artifacts outside the tagged text or structure element that contains meaningful content.",
        ),
        "7.1.2" => (
            "Meaningful tagged content is nested inside a region marked as decorative.",
            "Move meaningful content out of the artifact region and give it the appropriate structure tag.",
        ),
        "7.1.3" => (
            "Some page content is neither identified as decorative nor represented in the tagged structure.",
            "Tag meaningful content and mark only decorative graphics or page furniture as artifacts.",
        ),
        "7.1.4" => (
            "The PDF flags its accessibility structure as potentially suspect.",
            "Review and repair the document's tags, then regenerate it without carrying forward a stale Suspects flag.",
        ),
        "7.1.5" | "7.1.6" | "7.1.7" => (
            "A custom structure type is unmapped, mapped in a loop, or replaces a standard structure type.",
            "Remove invalid role-map entries and map custom tags to the closest standard PDF structure type without redefining standard tags.",
        ),
        "7.1.8" | "7.1.9" | "7.1.10" => (
            "Required document metadata or the viewer preference for displaying the title is missing or invalid.",
            "Add valid XMP metadata including a document title, and set the catalog preference to display that title.",
        ),
        "7.1.11" | "7.1.12" => (
            "The logical structure tree or a structure element's parent relationship is incomplete.",
            "Repair the tagged structure tree and its parent links using the PDF author's accessibility tagging tools.",
        ),
        "7.2" => (
            "A table, list, bookmark, annotation, form label, or language property has invalid structure or nesting.",
            "Use semantic elements with valid parent-child relationships, and set document or element language where the text needs it.",
        ),
        _ if clause.starts_with("7.2.") => (
            "A table, list, bookmark, annotation, form label, or language property has invalid structure or nesting.",
            "Use semantic elements with valid parent-child relationships, and set document or element language where the text needs it.",
        ),
        "7.3" => (
            "A figure has no usable text alternative describing its relevant information.",
            "Add concise alternative text to informative figures; mark purely decorative figures as artifacts.",
        ),
        "7.4.2" | "7.4.4" => (
            "Heading tags are skipped, repeated at one level, or mix incompatible heading hierarchies.",
            "Use one consistent heading scheme, keep levels in order, and give each section a clear heading structure.",
        ),
        "7.5" => (
            "Table header cells are missing or are not connected to the data they describe.",
            "Add header cells and valid header associations so assistive technology can identify each row and column.",
        ),
        "7.7" => (
            "A mathematical expression is not identified and described for nonvisual reading.",
            "Tag each formula and provide replacement text that conveys its meaning or spoken form.",
        ),
        "7.9" => (
            "A note or footnote is missing a unique identifier or shares one with another note.",
            "Assign each note a distinct identifier and keep its reference linked to the matching note in the structure tree.",
        ),
        "7.10" => (
            "An optional-content layer lacks a name or uses a prohibited visibility setting.",
            "Name each optional-content group and remove configuration entries that change layer visibility automatically.",
        ),
        "7.11" => (
            "An embedded file attachment is missing one or both required filename values.",
            "Give the attachment valid, non-empty file names in both the F and UF entries.",
        ),
        "7.15" => (
            "The PDF contains a dynamic XFA form, which is outside the PDF/UA-1 requirements.",
            "Replace the XFA form with a supported tagged form or provide a non-interactive accessible document.",
        ),
        "7.16" => (
            "The encryption permissions do not allow assistive technology to extract the document content.",
            "Change the security settings to permit accessibility and content extraction, or remove encryption.",
        ),
        _ if clause.starts_with("7.18.") => (
            "An annotation, link, media item, or form widget is missing required structure or descriptive data.",
            "Give each interactive item an appropriate tag, accessible description, and valid page tab order; remove unsupported annotation types.",
        ),
        "7.20" => (
            "A form XObject or reference XObject is not correctly represented in the tagged content structure.",
            "Associate each reusable graphic with the appropriate structure element and ensure referenced content is embedded.",
        ),
        "7.20.1" => (
            "A reference XObject points to content that is not embedded in the PDF.",
            "Embed the referenced graphic content directly in the document or remove the external reference.",
        ),
        "7.20.2" => (
            "A reusable form graphic is not connected to the document structure at each use.",
            "Ensure each use of the form XObject is represented in the structure tree and preserves its reading order.",
        ),
        _ if clause.starts_with("7.21.") => (
            "A font program, encoding, glyph inventory, or Unicode mapping is incomplete or inconsistent.",
            "Use an embedded font with valid glyph and width data and complete Unicode mappings, then regenerate the PDF.",
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
            "Repair the indicated file structure or regenerate the PDF with a PDF/A-capable writer; do not encrypt the output.",
        ),
        "2" => (
            "A page graphic uses a color space, image feature, or output intent that PDF/A-1 does not permit.",
            "Convert graphics to permitted color spaces and provide a valid output intent for the document's intended output.",
        ),
        "3" => (
            "A font program, descriptor, encoding, or character map is missing or inconsistent.",
            "Embed valid fonts with complete glyph and encoding data, and regenerate the document with font embedding enabled.",
        ),
        "4" => (
            "The document uses transparency in a graphics state, which PDF/A-1 does not allow.",
            "Flatten transparent artwork before export or produce the document under a profile that permits the required transparency.",
        ),
        "5" => (
            "An annotation type or annotation property is not permitted by PDF/A-1.",
            "Remove or flatten unsupported annotations and correct the flags, opacity, and appearance data of annotations that remain.",
        ),
        "6" => (
            "The document contains an action that PDF/A-1 prohibits.",
            "Remove the prohibited action or replace it with static content before exporting the archival PDF.",
        ),
        "7" => (
            "XMP metadata is missing, malformed, or inconsistent with the document information dictionary.",
            "Correct the required XMP properties and keep corresponding Info dictionary values synchronized.",
        ),
        "8" => (
            "The tagged structure, role mapping, or language information is incomplete or invalid.",
            "Repair the logical structure tree, map custom tags to standard types, and set valid language metadata.",
        ),
        "9" => (
            "The interactive form dictionary contains a setting PDF/A-1 does not permit.",
            "Disable automatic appearance generation and ensure each form field has a valid appearance stream.",
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
            "Repair the indicated file structure or regenerate the document with a PDF/A-capable writer; do not encrypt the output.",
        ),
        "2" => (
            "A graphic, color profile, image, or font uses data that the selected PDF/A profile does not permit.",
            "Correct the named object, embed valid color and font resources, and use supported image encodings and graphics operators.",
        ),
        "3" => (
            "An annotation type, flag, appearance, or description does not meet the PDF/A requirement.",
            "Remove or flatten unsupported annotations and repair the flags, appearance streams, and descriptions of those that remain.",
        ),
        "4" => (
            "An interactive form or signature dictionary contains a prohibited key or inconsistent signature data.",
            "Remove prohibited form actions and regenerate or validate signatures using a PDF/A-compatible signing workflow.",
        ),
        "5" => (
            "The document contains an action or additional-action entry that the selected PDF/A profile forbids.",
            "Remove the prohibited action from the catalog, page, annotation, or field and replace it with static content if needed.",
        ),
        "6" => (
            "XMP metadata is missing, malformed, or inconsistent with the document information dictionary.",
            "Correct required XMP properties and extension schemas, and keep corresponding Info dictionary values synchronized.",
        ),
        "7" => (
            "The tagged structure, role mapping, or language information is incomplete or invalid.",
            "Repair the logical structure tree, map custom tags to standard types, and set valid language metadata.",
        ),
        "8" => (
            "An embedded file lacks metadata or file-specification entries required by the selected PDF/A part.",
            "Add the required file name and MIME type, and associate the attachment with the document as the profile requires.",
        ),
        "9" => (
            "An optional-content group or configuration contains a name or setting disallowed by PDF/A.",
            "Name each layer and remove prohibited visibility or state entries from the optional-content configuration.",
        ),
        "10" => (
            "The document contains an alternate presentation entry not permitted by the selected PDF/A profile.",
            "Remove the alternate presentation from the document name dictionary and retain only static page content.",
        ),
        "11" => (
            "The document catalog contains a requirements entry that the selected PDF/A profile forbids.",
            "Remove the Requirements entry from the document catalog and regenerate the PDF.",
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
