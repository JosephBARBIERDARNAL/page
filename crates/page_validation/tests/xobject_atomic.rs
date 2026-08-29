pub mod common;

#[test]
fn multiple_invalid_xobjects_are_one_deterministic_unattached_failure() {
    let bytes = common::xobject_fixture("two_invalid_images");
    let first = common::validate(&bytes);
    let second = common::validate(&bytes);
    let first = common::assert_single_failure(&first, "PDFA1B-IMAGE-BPC-001");
    let second = common::assert_single_failure(&second, "PDFA1B-IMAGE-BPC-001");
    assert_eq!(first, second);
    assert!(first.object_id.is_none());
    assert!(first.message.contains("image"));
}
