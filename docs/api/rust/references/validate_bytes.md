# `validate_bytes`

**Function**

Validates PDF bytes already in memory against the profile declared in their own XMP metadata.

The document's XMP Identification schema is read to determine whether it targets PDF/A or PDF/UA and which part and conformance level apply, so the caller does not need to already know which profile to check against. Use [`validate_bytes_with_profile`] instead when the profile is already known or the document's own declaration should be ignored.

## Arguments

- `bytes` - The complete PDF file content.
- `limits` - The resource bounds enforced while parsing and inspecting the document.

## Returns

A [`ValidationReport`] describing which implemented checks for the declared profile passed or failed.

## Errors

Returns [`ValidationError::Pdf`] if parsing or inspecting the object graph fails or a [`SafetyLimits`] bound is exceeded, [`ValidationError::MissingProfileDeclaration`] or [`ValidationError::InvalidProfileDeclaration`] if the XMP metadata does not unambiguously declare one supported profile, and [`ValidationError::UnsupportedProfile`] if it declares a profile this crate does not implement yet.

## Examples

```
use page_validation::{SafetyLimits, validate_bytes};

let limits = SafetyLimits::default();
let error = validate_bytes(b"not a pdf", &limits).unwrap_err();
println!("{error}");
```
