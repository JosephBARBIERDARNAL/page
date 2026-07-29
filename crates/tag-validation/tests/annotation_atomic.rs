#[allow(dead_code)]
mod common;

const SUBTYPE: &str = "PDFA1B-ANNOTATION-SUBTYPE-001";
const OPACITY: &str = "PDFA1B-ANNOTATION-OPACITY-001";
const FLAGS: &str = "PDFA1B-ANNOTATION-FLAGS-001";
const COLOR: &str = "PDFA1B-ANNOTATION-COLOR-001";
const AP_ENTRIES: &str = "PDFA1B-ANNOTATION-AP-ENTRIES-001";
const BUTTON_AP: &str = "PDFA1B-WIDGET-BUTTON-APPEARANCE-001";
const OTHER_AP: &str = "PDFA1B-ANNOTATION-NORMAL-APPEARANCE-001";
const WIDGET_AP: &str = "PDFA1B-WIDGET-APPEARANCE-001";

const CASES: &[(&str, &[&str])] = &[
    ("subtype_widget", &[WIDGET_AP]),
    ("subtype_trapnet", &[]),
    ("subtype_file_attachment", &[SUBTYPE]),
    ("subtype_unknown", &[SUBTYPE]),
    ("subtype_missing", &[SUBTYPE]),
    ("direct_invalid_annotation", &[SUBTYPE]),
    ("unreferenced_invalid_annotation", &[]),
    ("opacity_absent", &[]),
    ("opacity_one", &[]),
    ("opacity_zero", &[OPACITY]),
    ("opacity_wrong_type", &[]),
    ("flags_missing", &[FLAGS]),
    ("flags_not_printable", &[FLAGS]),
    ("flags_invisible", &[FLAGS]),
    ("flags_hidden", &[FLAGS]),
    ("flags_no_view", &[FLAGS]),
    ("color_c_rgb", &[]),
    ("color_ic_rgb", &[]),
    ("color_c_cmyk", &[COLOR]),
    ("color_ic_without_output", &[COLOR]),
    ("no_color_cmyk", &[]),
    ("appearance_absent", &[]),
    ("appearance_n_stream", &[]),
    ("appearance_n_dictionary", &[OTHER_AP]),
    ("appearance_n_and_r", &[AP_ENTRIES]),
    ("appearance_empty", &[AP_ENTRIES]),
    ("appearance_wrong_type", &[]),
    ("widget_button_dictionary", &[]),
    ("widget_button_empty_dictionary", &[BUTTON_AP]),
    ("widget_button_stream", &[BUTTON_AP]),
    ("widget_text_stream", &[]),
    ("widget_inherited_button_dictionary", &[]),
];

#[test]
fn annotation_cases_have_the_complete_expected_failure_delta() {
    let baseline = common::failure_ids(&common::annotation_fixture("baseline"));
    for rule in [
        SUBTYPE, OPACITY, FLAGS, COLOR, AP_ENTRIES, BUTTON_AP, OTHER_AP, WIDGET_AP,
    ] {
        assert!(!baseline.contains(rule));
    }
    common::assert_case_deltas(common::annotation_fixture, "baseline", CASES);
}

#[test]
fn indirect_annotation_failure_attaches_the_annotation_object() {
    let report = common::validate(&common::annotation_fixture("flags_missing"));
    let failure = common::assert_single_failure(&report, FLAGS);
    assert!(failure.object_id.is_some());
}
