# `validate_bytes_with_profile`

**Function**

Validates PDF bytes already in memory against an explicitly selected profile, ignoring any profile the document declares in its own XMP metadata.

Unlike `validate_bytes`, this never fails outright: unreadable, malformed, or resource-exceeding input becomes an operational or parser `ValidationFailure` inside the returned report instead of an `Err`. Use `validate_bytes` instead when the caller wants an error, not a failing report, whenever the document does not declare `profile` itself.

## Arguments

- `bytes` - The complete PDF file content.
- `profile` - The PDF/A or PDF/UA profile to validate against, regardless of what the document's own XMP declares.
- `limits` - The resource bounds enforced while parsing and inspecting the document.

## Returns

A `ValidationReport` whose `checks_passed` is `true` only when every implemented check for `profile` passed, and whose `failures` explains every recorded problem, including operational ones such as an unimplemented `profile` or an exceeded `SafetyLimits` bound.

## Examples

```rs
use page_validation::{SafetyLimits, ValidationProfile, validate_bytes_with_profile};

let limits = SafetyLimits::default();
let report = validate_bytes_with_profile(b"not a pdf", ValidationProfile::PdfA1b, &limits);
assert_eq!(report.exit_code(), 2);
```
