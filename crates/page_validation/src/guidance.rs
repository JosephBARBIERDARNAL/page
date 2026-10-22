//! Provides possible causes and suggested fixes for reported conformance failures.
//!
//! Guidance is selected from the complete rule identifier and attached to each failure by the
//! validation pipeline. The wording is intentionally brief and advisory because a rule identifier
//! alone cannot determine the exact authoring mistake in a particular PDF.

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
        ("ISO 19005-2:2011:", Standard::PdfA2),
        ("ISO 19005-3:2012:", Standard::PdfA3),
    ]
    .into_iter()
    .find_map(|(prefix, standard)| {
        rule_id
            .strip_prefix(prefix)
            .map(|suffix| (standard, suffix))
    })?;

    let (clause, test_number) = suffix.rsplit_once(':')?;
    if !test_number.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    let known_rule_tests = match standard {
        Standard::PdfUa1 => PDFUA_RULE_TESTS,
        Standard::PdfA1 => PDFA1_RULE_TESTS,
        Standard::PdfA2 | Standard::PdfA3 => PDFA23_RULE_TESTS,
    };
    if !known_rule_test(known_rule_tests, clause, test_number)
        || (standard == Standard::PdfA2
            && clause == "6.8"
            && matches!(test_number, "1" | "3" | "4"))
        || (standard == Standard::PdfA3 && clause == "6.8" && test_number == "5")
    {
        return None;
    }

    let (possible_cause, suggested_fix) = match standard {
        Standard::PdfUa1 => pdfua_guidance(clause, test_number)?,
        Standard::PdfA1 => pdfa_1_guidance(clause)?,
        Standard::PdfA2 | Standard::PdfA3 => pdfa_2_or_3_guidance(clause)?,
    };

    Some(FailureGuidance::from_static(possible_cause, suggested_fix))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Standard {
    PdfUa1,
    PdfA1,
    PdfA2,
    PdfA3,
}

struct RuleTestSet {
    clause: &'static str,
    test_numbers: &'static str,
}

fn known_rule_test(rule_tests: &[RuleTestSet], clause: &str, test_number: &str) -> bool {
    rule_tests.iter().any(|rule_test| {
        rule_test.clause == clause
            && rule_test
                .test_numbers
                .split(',')
                .any(|known_test_number| known_test_number == test_number)
    })
}

const PDFUA_RULE_TESTS: &[RuleTestSet] = &[
    RuleTestSet {
        clause: "5",
        test_numbers: "1,2,3,4,5",
    },
    RuleTestSet {
        clause: "6.1",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.2",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "7.1",
        test_numbers: "1,2,3,4,5,6,7,8,9,10,11,12",
    },
    RuleTestSet {
        clause: "7.2",
        test_numbers: "2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,36,37,38,39,40,41,42,43",
    },
    RuleTestSet {
        clause: "7.3",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "7.4.2",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "7.4.4",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "7.5",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "7.7",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "7.9",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "7.10",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "7.11",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "7.15",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "7.16",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "7.18.1",
        test_numbers: "1,2,3",
    },
    RuleTestSet {
        clause: "7.18.2",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "7.18.3",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "7.18.4",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "7.18.5",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "7.18.6.2",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "7.18.8",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "7.20",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "7.21.3.1",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "7.21.3.2",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "7.21.3.3",
        test_numbers: "1,2,3",
    },
    RuleTestSet {
        clause: "7.21.4.1",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "7.21.4.2",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "7.21.5",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "7.21.6",
        test_numbers: "1,2,3,4",
    },
    RuleTestSet {
        clause: "7.21.7",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "7.21.8",
        test_numbers: "1",
    },
];

const PDFA1_RULE_TESTS: &[RuleTestSet] = &[
    RuleTestSet {
        clause: "6.1.10",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.1.11",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.1.12",
        test_numbers: "1,2,3,4,5,6,7,8,9,10",
    },
    RuleTestSet {
        clause: "6.1.13",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.1.2",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.1.3",
        test_numbers: "1,2,3,4",
    },
    RuleTestSet {
        clause: "6.1.4",
        test_numbers: "1,2,3",
    },
    RuleTestSet {
        clause: "6.1.6",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.1.7",
        test_numbers: "1,2,3",
    },
    RuleTestSet {
        clause: "6.1.8",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.2.10",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.2.2",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.2.3.2",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.2.3.3",
        test_numbers: "1,2,3",
    },
    RuleTestSet {
        clause: "6.2.4",
        test_numbers: "1,2,3,4,5",
    },
    RuleTestSet {
        clause: "6.2.5",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.2.6",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.2.7",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.2.8",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.2.9",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.3.2",
        test_numbers: "1,2,3,4,5,6,7",
    },
    RuleTestSet {
        clause: "6.3.3.1",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.3.3.2",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.3.3.3",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.3.4",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.3.5",
        test_numbers: "1,2,3",
    },
    RuleTestSet {
        clause: "6.3.6",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.3.7",
        test_numbers: "1,2,3",
    },
    RuleTestSet {
        clause: "6.3.8",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.4",
        test_numbers: "1,2,3,4,5,6",
    },
    RuleTestSet {
        clause: "6.5.2",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.5.3",
        test_numbers: "1,2,3,4,5,6",
    },
    RuleTestSet {
        clause: "6.6.1",
        test_numbers: "1,2,3",
    },
    RuleTestSet {
        clause: "6.6.2",
        test_numbers: "1,2,3",
    },
    RuleTestSet {
        clause: "6.7.11",
        test_numbers: "1,2,3,4,5,6",
    },
    RuleTestSet {
        clause: "6.7.2",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.7.3",
        test_numbers: "1,2,3,4,5,6,7,8",
    },
    RuleTestSet {
        clause: "6.7.5",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.7.8",
        test_numbers: "1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19",
    },
    RuleTestSet {
        clause: "6.7.9",
        test_numbers: "1,2,3",
    },
    RuleTestSet {
        clause: "6.8.2.2",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.8.3.3",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.8.3.4",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.8.4",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.9",
        test_numbers: "1,2",
    },
];

const PDFA23_RULE_TESTS: &[RuleTestSet] = &[
    RuleTestSet {
        clause: "6.1.10",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.1.12",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.1.13",
        test_numbers: "1,2,3,4,5,7,8,9,10,11",
    },
    RuleTestSet {
        clause: "6.1.2",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.1.3",
        test_numbers: "1,2,3",
    },
    RuleTestSet {
        clause: "6.1.4",
        test_numbers: "2",
    },
    RuleTestSet {
        clause: "6.1.6",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.1.7.1",
        test_numbers: "1,2,3",
    },
    RuleTestSet {
        clause: "6.1.7.2",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.1.8",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.1.9",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.10",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.11",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.2.10",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.2.11.2",
        test_numbers: "1,2,3,4,5,6,7",
    },
    RuleTestSet {
        clause: "6.2.11.3.1",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.2.11.3.2",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.2.11.3.3",
        test_numbers: "1,2,3",
    },
    RuleTestSet {
        clause: "6.2.11.4.1",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.2.11.4.2",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.2.11.5",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.2.11.6",
        test_numbers: "1,2,3,4",
    },
    RuleTestSet {
        clause: "6.2.11.7.2",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.2.11.7.3",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.2.11.8",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.2.2",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.2.3",
        test_numbers: "1,2,3",
    },
    RuleTestSet {
        clause: "6.2.4.2",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.2.4.3",
        test_numbers: "2,3,4",
    },
    RuleTestSet {
        clause: "6.2.4.4",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.2.5",
        test_numbers: "1,2,3,4,5,6",
    },
    RuleTestSet {
        clause: "6.2.6",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.2.8",
        test_numbers: "1,2,3,4,5",
    },
    RuleTestSet {
        clause: "6.2.8.3",
        test_numbers: "1,2,3,4,5",
    },
    RuleTestSet {
        clause: "6.2.9",
        test_numbers: "1,2,3",
    },
    RuleTestSet {
        clause: "6.3.1",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.3.2",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.3.3",
        test_numbers: "1,2,3,4",
    },
    RuleTestSet {
        clause: "6.4.1",
        test_numbers: "1,2,3",
    },
    RuleTestSet {
        clause: "6.4.2",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.4.3",
        test_numbers: "1,2,3",
    },
    RuleTestSet {
        clause: "6.5.1",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.5.2",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.6.2.1",
        test_numbers: "1,2,3,4,5",
    },
    RuleTestSet {
        clause: "6.6.2.3.1",
        test_numbers: "1,2",
    },
    RuleTestSet {
        clause: "6.6.2.3.2",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.6.2.3.3",
        test_numbers: "1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18",
    },
    RuleTestSet {
        clause: "6.6.4",
        test_numbers: "1,2,3,4,5,6,7",
    },
    RuleTestSet {
        clause: "6.7.2.2",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.7.3.3",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.7.3.4",
        test_numbers: "1,2,3",
    },
    RuleTestSet {
        clause: "6.7.4",
        test_numbers: "1",
    },
    RuleTestSet {
        clause: "6.8",
        test_numbers: "1,2,3,4,5",
    },
    RuleTestSet {
        clause: "6.9",
        test_numbers: "1,2,3,4",
    },
];

fn pdfua_guidance(clause: &str, test_number: &str) -> Option<(&'static str, &'static str)> {
    Some(match (clause, test_number) {
        ("5", "1") => (
            "The PDF/UA identification metadata is absent, malformed, or inconsistent with PDF/UA-1",
            "Correct the PDF/UA identification properties in the exported XMP metadata",
        ),
        ("5", "2") => (
            "The PDF/UA identification part is not a single value of 1",
            "Set pdfuaid:part to the single integer value 1",
        ),
        ("5", "3" | "4" | "5") => (
            "A PDF/UA identification property uses a namespace prefix other than pdfuaid",
            "Use the pdfuaid prefix for the part, amd, and corr identification properties",
        ),
        ("6.1", "1") => (
            "The file header does not use a PDF version and line ending accepted by the standard",
            "Export with a PDF writer that produces a supported PDF header",
        ),
        ("6.2", "1") => (
            "The catalog does not clearly mark the file as a tagged PDF",
            "Enable tagged PDF output and set MarkInfo/Marked to true",
        ),
        ("7.1", "1") => (
            "Artifact content is nested inside tagged content",
            "Move artifacts outside tagged content and keep them in the page content stream",
        ),
        ("7.1", "2") => (
            "Tagged content is nested inside an artifact",
            "Move meaningful content out of the artifact and assign its appropriate structure tag",
        ),
        ("7.1", "3") => (
            "Page content is neither tagged nor marked as an artifact",
            "Tag meaningful content and mark only decorative graphics or page furniture as artifacts",
        ),
        ("7.1", "4") => (
            "The PDF flags its accessibility structure as potentially suspect",
            "Review the tags and clear the Suspects flag if it is no longer accurate",
        ),
        ("7.1", "5") => (
            "A custom structure type does not resolve through the RoleMap to a standard type",
            "Add a valid RoleMap entry from each custom type to an appropriate standard structure type",
        ),
        ("7.1", "6") => (
            "The structure tree RoleMap contains a circular mapping",
            "Remove or repair the RoleMap entries that create the cycle",
        ),
        ("7.1", "7") => (
            "A standard structure type is remapped in the RoleMap",
            "Remove the mapping that redefines the standard structure type",
        ),
        ("7.1", "8") => (
            "The catalog Metadata entry is absent or does not resolve to a valid XML metadata stream",
            "Add a catalog Metadata stream with Type Metadata and Subtype XML",
        ),
        ("7.1", "9") => (
            "The XMP metadata does not contain the document title",
            "Add a dc:title property to the XMP metadata",
        ),
        ("7.1", "10") => (
            "Viewer preferences do not enable display of the document title",
            "Set viewer preferences to display the document title and ensure the XMP title is present",
        ),
        ("7.1", "11") => (
            "The catalog does not contain a StructTreeRoot for the logical structure hierarchy",
            "Create a structure tree root and populate it with the document's semantic structure",
        ),
        ("7.1", "12") => (
            "A structure element is missing its required parent link",
            "Repair the structure tree so every structure element has a valid parent relationship",
        ),
        (
            "7.2",
            "2" | "21" | "22" | "23" | "24" | "25" | "29" | "30" | "31" | "32" | "33" | "34",
        ) => (
            "A document, bookmark, annotation, form field, or content element is missing a required natural-language value",
            "Set the document language and add a valid language value to the affected structure element or property",
        ),
        (
            "7.2",
            "3" | "4" | "5" | "6" | "7" | "8" | "9" | "10" | "11" | "12" | "13" | "14" | "15"
            | "16" | "36" | "37" | "38" | "39" | "41" | "42" | "43",
        ) => (
            "A table, row, or cell has invalid structure, children, or span relationships",
            "Repair the table hierarchy, cell associations, captions, and row or column spans",
        ),
        ("7.2", "17" | "18" | "19" | "20" | "40") => (
            "A list, list item, or list body has invalid structure or caption placement",
            "Use the required L, LI, and LBody nesting and place any caption in a permitted position",
        ),
        ("7.2", "26" | "27" | "28") => (
            "A table of contents or one of its entries has invalid structure or caption placement",
            "Use the required TOC and TOCI hierarchy and place any caption in a permitted position",
        ),
        ("7.3", "1") => (
            "A figure lacks a usable text alternative",
            "Add concise alternative text to informative figures; mark decorative figures as artifacts",
        ),
        ("7.4.2", "1") => (
            "Heading elements are nested in an invalid sequence",
            "Arrange heading tags in a consistent hierarchy without skipping levels",
        ),
        ("7.4.4", "1") => (
            "A structure element contains multiple H children",
            "Use a single H child at that level or separate the content into appropriate sections",
        ),
        ("7.4.4", "2") => (
            "Headings skip levels, repeat at a level, or mix hierarchies",
            "Use one consistent hierarchy with ordered levels and clear section headings",
        ),
        ("7.5", "1") => (
            "The header cells for one or more table data cells cannot be determined",
            "Add header cells and specify their associations with the data cells they describe",
        ),
        ("7.5", "2") => (
            "A table data cell refers to a header identifier that is not defined",
            "Define the referenced header cells and use unique IDs for the table's headers",
        ),
        ("7.7", "1") => (
            "A formula lacks a description for nonvisual reading",
            "Tag formulas and provide replacement text conveying their meaning or spoken form",
        ),
        ("7.9", "1") => (
            "A Note structure element is missing its required ID",
            "Assign an ID to the note and associate each reference with that note in the structure tree",
        ),
        ("7.9", "2") => (
            "Two or more Note structure elements share the same ID",
            "Assign a unique ID to each note and update its references in the structure tree",
        ),
        ("7.10", "1") => (
            "An optional-content group is missing its required name",
            "Set a non-empty Name on each optional-content group",
        ),
        ("7.10", "2") => (
            "An optional-content configuration uses a prohibited automatic-visibility setting",
            "Remove the prohibited visibility action or setting from the optional-content configuration",
        ),
        ("7.11", "1") => (
            "An attachment is missing a required filename value",
            "Set non-empty filenames in both the F and UF entries",
        ),
        ("7.15", "1") => (
            "The PDF contains a dynamic XFA form, which PDF/UA-1 does not support",
            "Replace XFA with a tagged form or an accessible non-interactive document",
        ),
        ("7.16", "1") => (
            "Encryption prevents assistive technology from extracting document content",
            "Allow accessibility and content extraction in the security settings, or remove encryption",
        ),
        ("7.18.1", "1") => (
            "An annotation is not nested in an Annot structure element",
            "Add an Annot structure element for the annotation and connect it to the page's structure tree",
        ),
        ("7.18.1", "2") => (
            "An annotation is missing both Contents and alternative text",
            "Add a concise Contents value or alternative text that describes the annotation",
        ),
        ("7.18.1", "3") => (
            "A form widget is missing its accessible field name",
            "Set the field's TU value or provide equivalent alternative text",
        ),
        ("7.18.2", "1") => (
            "The PDF contains a TrapNet annotation that is not permitted for PDF/UA-1",
            "Remove the TrapNet annotation or replace it with supported static content",
        ),
        ("7.18.3", "1") => (
            "A page does not define a valid tab order for its annotations",
            "Set the page's Tabs entry to Structure so keyboard order follows the structure tree",
        ),
        ("7.18.4", "1") => (
            "A form widget is not nested in a Form structure element",
            "Place the widget under a Form structure element and preserve its logical reading order",
        ),
        ("7.18.4", "2") => (
            "A Form structure element has invalid children or lacks a valid form role",
            "Give the form element a standard Form role and repair its child structure",
        ),
        ("7.18.5", "1") => (
            "A Link annotation is not nested in a Link structure element",
            "Add a Link structure element and associate the annotation with its link text",
        ),
        ("7.18.5", "2") => (
            "A Link annotation is missing its Contents description",
            "Set a concise Contents value that describes the link destination or purpose",
        ),
        ("7.18.6.2", "1") => (
            "A media clip is missing its required content type",
            "Set the media clip's CT entry to the correct content type",
        ),
        ("7.18.6.2", "2") => (
            "A media clip is missing alternative text",
            "Add alternative text that identifies the media clip and conveys its purpose",
        ),
        ("7.18.8", "1") => (
            "A PrinterMark annotation is not identified as incidental",
            "Mark the PrinterMark annotation as incidental or remove it if it is unnecessary",
        ),
        ("7.20", "1") => (
            "A reference XObject is not represented in the logical structure",
            "Associate the reference XObject with the appropriate structure element",
        ),
        ("7.20", "2") => (
            "A Form XObject with marked content is reused, so its structure parent is ambiguous",
            "Avoid reusing the tagged Form XObject or give each use a distinct structure representation",
        ),
        ("7.21.3.1", "1") => (
            "A Type 0 font has incompatible CIDSystemInfo data",
            "Use matching CIDSystemInfo registry and ordering values for the font and its CMap",
        ),
        ("7.21.3.2", "1") => (
            "A CID font has a missing or invalid CIDToGIDMap",
            "Provide a valid CIDToGIDMap or use Identity when the glyph mapping is identity",
        ),
        ("7.21.3.3", "1") => (
            "A Type 0 font uses an embedded CMap that is missing from the PDF",
            "Embed every non-predefined CMap referenced by the font",
        ),
        ("7.21.3.3", "2") => (
            "An embedded CMap has a WMode that conflicts with the font's encoding",
            "Correct the CMap WMode so it matches the Type 0 font encoding",
        ),
        ("7.21.3.3", "3") => (
            "A Type 0 font uses a CMap name that is not permitted by PDF/UA-1",
            "Use a predefined CMap name permitted for the font's character collection",
        ),
        ("7.21.4.1", "1") => (
            "A font used for page content is not embedded or has an invalid font program",
            "Embed a valid font program for every font used by page content",
        ),
        ("7.21.4.1", "2") => (
            "A used character has no corresponding glyph in the embedded font program",
            "Use a font program that contains every glyph used in the document",
        ),
        ("7.21.4.2", "1") => (
            "A subset Type 1 font has an invalid CharSet entry",
            "Make the CharSet entry list the glyph names actually present in the subset font",
        ),
        ("7.21.4.2", "2") => (
            "A subset CID font has an invalid CIDSet stream",
            "Regenerate the CIDSet so it correctly identifies the CIDs present in the subset font",
        ),
        ("7.21.5", "1") => (
            "A font's declared widths do not match the embedded font program",
            "Correct the PDF width entries or regenerate the PDF with consistent embedded font metrics",
        ),
        ("7.21.6", "1") => (
            "A non-symbolic TrueType font has no usable non-symbol character map",
            "Embed a TrueType font with the required Unicode character map",
        ),
        ("7.21.6", "2") => (
            "A non-symbolic TrueType font lacks the required Microsoft Unicode character map",
            "Use a font with a Microsoft Unicode cmap and a compatible encoding",
        ),
        ("7.21.6", "3") => (
            "A symbolic TrueType font defines an Encoding entry that is not permitted",
            "Remove the font dictionary Encoding entry from the symbolic TrueType font",
        ),
        ("7.21.6", "4") => (
            "A symbolic TrueType font has an invalid character map configuration",
            "Correct the font's cmap subtables and retain only combinations allowed by PDF/UA-1",
        ),
        ("7.21.7", "1") => (
            "A used glyph cannot be mapped to the correct Unicode character",
            "Correct the font's Unicode mapping so each used glyph maps to its intended text",
        ),
        ("7.21.7", "2") => (
            "A ToUnicode mapping contains a prohibited Unicode value",
            "Replace null, byte-order-mark, or noncharacter mappings with the correct Unicode text",
        ),
        ("7.21.8", "1") => (
            "A text-showing operation uses a .notdef glyph",
            "Correct the font encoding or text so every shown character resolves to a defined glyph",
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
            "The PDF header, object syntax, trailer, or cross-reference data is missing or malformed for PDF/A",
            "Repair the file structure or regenerate with a PDF/A writer; remove encryption",
        ),
        "2" => (
            "A page graphic uses a color space, image feature, or output intent that PDF/A-1 does not permit",
            "Convert graphics to permitted color spaces and add a valid output intent",
        ),
        "3" => (
            "A font program, descriptor, encoding, or character map is missing or invalid",
            "Embed fonts with valid glyph and encoding data",
        ),
        "4" => (
            "The document uses transparency in a graphics state, which PDF/A-1 does not allow",
            "Flatten transparency before export or use a profile that permits it",
        ),
        "5" => (
            "An annotation type or property is not permitted by PDF/A-1",
            "Remove or flatten unsupported annotations; correct remaining flags, opacity, and appearances",
        ),
        "6" => (
            "The document contains an action that PDF/A-1 prohibits",
            "Remove the action or replace it with static content",
        ),
        "7" => (
            "XMP is missing, malformed, or inconsistent with the Info dictionary",
            "Correct required XMP properties and synchronize matching Info values",
        ),
        "8" => (
            "The tagged structure, role mapping, or language information is incomplete or invalid",
            "Repair the structure tree, map custom tags to standard types, and set the document language",
        ),
        "9" => (
            "The interactive form dictionary contains a setting PDF/A-1 does not permit",
            "Disable automatic appearance generation and provide an appearance stream for each field",
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
            "The PDF header, object syntax, trailer, or cross-reference data is missing or malformed for PDF/A",
            "Repair the file structure or regenerate with a PDF/A writer; remove encryption",
        ),
        "2" => (
            "A graphic, color profile, image, or font uses data the selected PDF/A profile forbids",
            "Correct the object and use supported color, font, image, and graphics data",
        ),
        "3" => (
            "An annotation's type, flag, appearance, or description violates PDF/A",
            "Remove or flatten unsupported annotations; repair remaining flags, appearances, and descriptions",
        ),
        "4" => (
            "An interactive form or signature dictionary contains a prohibited key or inconsistent signature data",
            "Remove prohibited keys and repair signatures with a PDF/A-compatible workflow",
        ),
        "5" => (
            "The document contains an action or additional-action entry that the selected PDF/A profile forbids",
            "Remove the prohibited action; replace it with static content if needed",
        ),
        "6" => (
            "XMP is missing, malformed, or inconsistent with the Info dictionary",
            "Correct required XMP properties and extension schemas; synchronize matching Info values",
        ),
        "7" => (
            "The tagged structure, role mapping, or language information is incomplete or invalid",
            "Repair the structure tree, map custom tags to standard types, and set the document language",
        ),
        "8" => (
            "An embedded file lacks metadata or file-specification entries required by the selected PDF/A part",
            "Add the required filename and MIME type, and associate the attachment as required",
        ),
        "9" => (
            "An optional-content group or configuration has a name or setting disallowed by PDF/A",
            "Name each layer and remove prohibited visibility settings",
        ),
        "10" => (
            "The document contains an alternate presentation entry not permitted by the selected PDF/A profile",
            "Remove the alternate presentation and retain static page content",
        ),
        "11" => (
            "The document catalog contains a requirements entry that the selected PDF/A profile forbids",
            "Remove the Requirements entry from the catalog",
        ),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        PDFA1_RULE_TESTS, PDFA23_RULE_TESTS, PDFUA_RULE_TESTS, RuleTestSet, guidance_for_rule_id,
    };

    #[test]
    fn provides_guidance_for_pdfua_and_pdfa_failures() {
        for rule_id in [
            "ISO 14289-1:2014:5:1",
            "ISO 14289-1:2014:7.2:29",
            "ISO 14289-1:2014:7.21.7:2",
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
    fn covers_every_emitted_rule_test() {
        assert_rule_test_coverage("ISO 14289-1:2014:", PDFUA_RULE_TESTS, &[]);
        assert_rule_test_coverage("ISO 19005-1:2005:", PDFA1_RULE_TESTS, &[]);
        assert_rule_test_coverage(
            "ISO 19005-2:2011:",
            PDFA23_RULE_TESTS,
            &[("6.8", "1"), ("6.8", "3"), ("6.8", "4")],
        );
        assert_rule_test_coverage("ISO 19005-3:2012:", PDFA23_RULE_TESTS, &[("6.8", "5")]);
    }

    fn assert_rule_test_coverage(
        prefix: &str,
        rule_tests: &[RuleTestSet],
        excluded_rule_tests: &[(&str, &str)],
    ) {
        for rule_test in rule_tests {
            for test_number in rule_test.test_numbers.split(',') {
                if excluded_rule_tests.contains(&(rule_test.clause, test_number)) {
                    continue;
                }
                let rule_id = format!("{prefix}{}:{test_number}", rule_test.clause);
                let guidance = guidance_for_rule_id(&rule_id)
                    .unwrap_or_else(|| panic!("missing guidance for {rule_id}"));
                assert!(
                    !guidance.possible_cause.is_empty(),
                    "empty cause for {rule_id}"
                );
                assert!(
                    !guidance.suggested_fix.is_empty(),
                    "empty fix for {rule_id}"
                );
            }
        }
    }

    #[test]
    fn distinguishes_pdfua_annotation_and_xobject_tests() {
        let link_structure =
            guidance_for_rule_id("ISO 14289-1:2014:7.18.5:1").expect("link structure guidance");
        let link_description =
            guidance_for_rule_id("ISO 14289-1:2014:7.18.5:2").expect("link description guidance");
        assert_ne!(link_structure, link_description);
        assert!(
            link_structure
                .suggested_fix
                .contains("Link structure element")
        );
        assert!(link_description.suggested_fix.contains("Contents"));

        let reference_xobject =
            guidance_for_rule_id("ISO 14289-1:2014:7.20:1").expect("reference XObject guidance");
        let reused_form =
            guidance_for_rule_id("ISO 14289-1:2014:7.20:2").expect("Form XObject guidance");
        assert_ne!(reference_xobject, reused_form);
        assert!(
            reference_xobject
                .possible_cause
                .contains("reference XObject")
        );
        assert!(reused_form.possible_cause.contains("Form XObject"));
    }

    #[test]
    fn leaves_unknown_rule_identifiers_without_guidance() {
        assert!(guidance_for_rule_id("RULE-UNKNOWN").is_none());
        assert!(guidance_for_rule_id("ISO 19005-4:2020:6.1:1").is_none());
        assert!(guidance_for_rule_id("ISO 14289-1:2014:7.22:1").is_none());
        assert!(guidance_for_rule_id("ISO 14289-1:2014:7.18.5:3").is_none());
        assert!(guidance_for_rule_id("ISO 14289-1:2014:7.18.5:").is_none());
        assert!(guidance_for_rule_id("ISO 14289-1:2014:7.18.5:one").is_none());
        assert!(guidance_for_rule_id("ISO 14289-1:2014:7.20.1:1").is_none());
        assert!(guidance_for_rule_id("ISO 19005-2:2011:6.8:1").is_none());
        assert!(guidance_for_rule_id("ISO 19005-3:2012:6.8:5").is_none());
    }
}
