# `validate_file`

**Function**

Reads a file from disk and validates it against the profile declared in its own XMP metadata.

This is the file-based counterpart of `validate_bytes`: it enforces `limits.max_input_size` against the file's size before reading it into memory, then delegates to `validate_bytes`. The returned report has its `source` set to `path`.

## Arguments

- `path` - The PDF file to read and validate.
- `limits` - The resource bounds enforced while reading, parsing, and inspecting the document.

## Returns

A `ValidationReport` describing which implemented checks for the declared profile passed or failed, with `source` set to `path`.

## Errors

Returns `ValidationError::InputIo` if `path` cannot be read or its size cannot be determined, and every error `validate_bytes` can return once the file content is available, including an oversized file reported as `PdfError::InputTooLarge`.

## Examples

```rs
use std::path::Path;

use page_validation::{SafetyLimits, validate_file};

let limits = SafetyLimits::default();
let report = validate_file(Path::new("input.pdf"), &limits)?;
println!("{report}");
# Ok::<(), page_validation::ValidationError>(())
```
