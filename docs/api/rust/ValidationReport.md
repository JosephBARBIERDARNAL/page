# `ValidationReport`

**Struct**

The outcome of validating one document against one `ValidationProfile`: whether it passed, how many checks ran, and every recorded `ValidationFailure`.

`checks_passed` is `true` only when every implemented check for `profile` passed; `preliminary` marks the result as based on this crate's still-growing rule subset rather than full veraPDF conformance. `document` holds the normalized document used during validation, or `None` when validation stopped before one could be built. Use `Self::exit_code` to translate a report into the process exit status this crate's CLI relies on, and `Self::has_operational_failure` to check whether any recorded failure is `FailureCategory::Operational` rather than a conformance finding.

## Examples

```rs
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes};

let limits = SafetyLimits::default();
let error = validate_bytes(b"not a pdf", Some(ValidationProfile::PdfA1b), &limits)
    .expect_err("malformed input");
assert!(matches!(error, page_validation::ValidationError::Pdf(_)));
```
