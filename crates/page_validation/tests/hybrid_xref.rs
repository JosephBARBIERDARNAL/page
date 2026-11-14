use std::io::Write as _;

use page_validation::{
    SafetyLimits, ValidationErrorKind, ValidationOptions, ValidationProfile, validate_pdf_bytes,
};

const OLDER_HYBRID: &[u8] = include_bytes!("fixtures/hybrid-older-structure.pdf");
const FIGURE_ALT: &str = "ISO 14289-1:2014:7.3:1";
const STRUCTURE_LANGUAGE: &str = "ISO 14289-1:2014:7.2:22";

fn failed_rules(bytes: &[u8]) -> Vec<String> {
    validate_pdf_bytes(
        bytes,
        &ValidationOptions::default().profile(ValidationProfile::PdfUa1),
    )
    .expect("inspect compressed structure tree")
    .failures
    .into_iter()
    .map(|failure| failure.rule_id)
    .collect()
}

fn final_xref(bytes: &[u8]) -> usize {
    std::str::from_utf8(
        bytes
            .rsplit(|byte| *byte == b'\n')
            .nth(2)
            .expect("final startxref offset"),
    )
    .expect("ASCII offset")
    .parse()
    .expect("numeric offset")
}

fn append_revision(bytes: &mut Vec<u8>, previous: usize, entries: &str, extra: &str) {
    let offset = bytes.len();
    writeln!(
        bytes,
        "xref\n{entries}trailer\n<< /Size 9 /Root 1 0 R /Prev {previous} {extra} >>\nstartxref\n{offset}\n%%EOF"
    )
    .expect("write revision");
}

#[test]
fn older_and_current_hybrid_sections_produce_the_same_structure_findings() {
    let older = failed_rules(OLDER_HYBRID);
    let current = failed_rules(
        OLDER_HYBRID
            .get(..final_xref(OLDER_HYBRID))
            .expect("remove final update"),
    );
    assert_eq!(older, current);
    for rule in [FIGURE_ALT, STRUCTURE_LANGUAGE] {
        assert!(older.iter().any(|failed| failed == rule), "missing {rule}");
    }
}

#[test]
fn older_hybrid_stream_replaces_free_container_and_member_placeholders() {
    let mut bytes = OLDER_HYBRID.to_vec();
    bytes.truncate(final_xref(&bytes));
    bytes.truncate(final_xref(&bytes));
    let previous = final_xref(&bytes);
    let stream_offset = bytes
        .windows(b"8 0 obj".len())
        .position(|window| window == b"8 0 obj")
        .expect("hybrid xref stream");
    append_revision(
        &mut bytes,
        previous,
        "5 3\n0000000000 00000 f \n0000000000 00000 f \n0000000000 00000 f \n",
        &format!("/XRefStm {stream_offset}"),
    );
    let previous = final_xref(&bytes);
    append_revision(&mut bytes, previous, "0 0\n", "");

    let rules = failed_rules(&bytes);
    assert!(rules.iter().any(|rule| rule == FIGURE_ALT));
    assert_eq!(rules, failed_rules(OLDER_HYBRID));
}

#[test]
fn newer_replacement_or_deletion_overrides_an_older_compressed_figure() {
    for replacement in [true, false] {
        let mut bytes = OLDER_HYBRID.to_vec();
        let previous = final_xref(&bytes);
        let entries = if replacement {
            let offset = bytes.len();
            bytes.extend_from_slice(b"6 0 obj\n<< /Type /StructElem /S /Figure /P 5 0 R /Alt (A figure) /K [] >>\nendobj\n");
            format!("6 1\n{offset:010} 00000 n \n")
        } else {
            "6 1\n0000000000 00001 f \n".to_owned()
        };
        append_revision(&mut bytes, previous, &entries, "");

        let rules = failed_rules(&bytes);
        assert!(!rules.iter().any(|rule| rule == FIGURE_ALT));
        assert_eq!(
            rules.iter().any(|rule| rule == STRUCTURE_LANGUAGE),
            replacement,
        );
    }
}

#[test]
fn older_hybrid_objects_remain_subject_to_safety_limits() {
    for limits in [
        SafetyLimits::default().with_max_object_count(5),
        SafetyLimits::default().with_max_decoded_stream_size(20),
        SafetyLimits::default().with_max_xref_revisions(2),
        SafetyLimits::default().with_max_reference_depth(0),
    ] {
        let error = validate_pdf_bytes(
            OLDER_HYBRID,
            &ValidationOptions::default()
                .profile(ValidationProfile::PdfUa1)
                .limits(limits),
        )
        .expect_err("enforce hybrid object safety limits");
        assert_eq!(error.kind(), ValidationErrorKind::SafetyLimit);
    }
}
