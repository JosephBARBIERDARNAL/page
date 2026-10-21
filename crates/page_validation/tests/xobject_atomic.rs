pub mod common;

#[test]
fn multiple_invalid_xobjects_are_one_deterministic_failure() {
    let bytes = common::xobject_fixture("two_invalid_images");
    let first = common::validate(&bytes);
    let second = common::validate(&bytes);
    let first = common::assert_single_failure(&first, "ISO 19005-1:2005:6.2.4:4");
    let second = common::assert_single_failure(&second, "ISO 19005-1:2005:6.2.4:4");
    assert_eq!(first, second);
    assert!(first.message.contains("image"));
}
