# `ValidationError`

**Enum**

The top-level error returned by `validate_bytes` and `validate_file` when a document cannot be scored against a profile at all.

This is distinct from a `ValidationReport` recording failures: a report means the profile's rules ran and found conformance problems, while `ValidationError` means the input could not be read, parsed, or matched to a profile in the first place. `Self::Pdf` carries the lower-level `PdfError` from parsing or inspecting the object graph.

## Examples

```rs
use page_validation::{SafetyLimits, ValidationError, validate_bytes};

let limits = SafetyLimits::default();
let error = validate_bytes(b"not a pdf", &limits).unwrap_err();
assert!(matches!(error, ValidationError::Pdf(_)));
```
