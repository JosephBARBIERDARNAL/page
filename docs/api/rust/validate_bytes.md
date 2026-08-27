# `validate_bytes`

**Function**

Validates PDF bytes already in memory against a selected profile.

Pass `None` for `profile` to infer the profile from the document's XMP Identification schema, or `Some(profile)` to validate against that profile regardless of the declaration.

## Arguments

- `bytes` - The complete PDF file content.
- `profile` - An explicit validation profile, or `None` to infer it from XMP metadata.
- `limits` - The resource bounds enforced while parsing and inspecting the document.

## Returns

A `ValidationReport` describing which implemented checks for the selected profile passed or failed.

## Errors

Returns `ValidationError::Pdf` if parsing or inspecting the object graph fails or a `SafetyLimits` bound is exceeded, `ValidationError::MissingProfileDeclaration` or `ValidationError::InvalidProfileDeclaration` if `profile` is `None` and XMP does not unambiguously declare a profile, and `ValidationError::UnsupportedProfile` if the selected profile is not implemented yet.

## Examples

```rs
use page_validation::{SafetyLimits, validate_bytes};

let limits = SafetyLimits::default();
let error = validate_bytes(b"not a pdf", None, &limits).unwrap_err();
println!("{error}");
```
