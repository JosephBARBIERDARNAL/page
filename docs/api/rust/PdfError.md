# `PdfError`

**Enum**

Errors from parsing a PDF or inspecting its object graph before the validation rules run.

Each variant is either a strict-parser rejection (for example a malformed cross-reference table) or one of the configurable `SafetyLimits` bounds being exceeded, such as an oversized input or an over-deep reference chain. `ValidationError::Pdf` wraps this type for the public `validate_bytes` and `validate_file` entry points, so callers that only need the top-level outcome can match on `ValidationError` instead.

## Examples

```rs
use page_validation::{SafetyLimits, ValidationError, validate_bytes};

let limits = SafetyLimits {
    max_input_size: 4,
    ..SafetyLimits::default()
};
let error = validate_bytes(b"%PDF-1.4", None, &limits).unwrap_err();
assert!(matches!(error, ValidationError::Pdf(_)));
```
