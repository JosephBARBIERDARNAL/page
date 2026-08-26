# `ValidationFailure`

**Struct**

One recorded conformance, metadata, parser, or operational problem in a [`ValidationReport`].

`rule_id` identifies the specific check (for example `PDFA1B-CATALOG-001`), `message` is a human-readable description, `object_id` is the indirect object the failure is attributed to when one applies, and `category` classifies the failure via [`FailureCategory`]. Multiple raw findings for the same rule are aggregated into as few `ValidationFailure` values as the rule allows before being placed in [`ValidationReport::failures`].

## Examples

```
use page_validation::{FailureCategory, ValidationFailure};

let failure = ValidationFailure {
    rule_id: "PDFA1B-CATALOG-001".to_owned(),
    message: "document trailer does not resolve to a Catalog dictionary".to_owned(),
    object_id: None,
    category: FailureCategory::Conformance,
};
assert_eq!(failure.category, FailureCategory::Conformance);
```
