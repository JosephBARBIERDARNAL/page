# `validate_file`

**Function**

Reads a file from disk and validates it against a selected profile.

Pass `None` for `profile` to infer the profile from the document's XMP metadata, or `Some(profile)` to validate against that profile regardless of the declaration. This is the file-based counterpart of [`validate_bytes`]. It enforces `limits.max_input_size` against the file's size before reading it into memory, then delegates to `validate_bytes`. The returned report has its `source` set to `path`.

## Arguments

- `path` - The PDF file to read and validate.
- `profile` - An explicit validation profile, or `None` to infer it from XMP metadata.
- `limits` - The resource bounds enforced while reading, parsing, and inspecting the document.

## Returns

A `ValidationReport` describing which implemented checks for the selected profile passed or failed, with `source` set to `path`.

## Errors

Returns `ValidationError::InputIo` if `path` cannot be read or its size cannot be determined, every parser or safety-limit error `validate_bytes` can return once the file content is available, and a profile-declaration error when `profile` is `None` and XMP does not unambiguously declare an implemented profile.

## Examples

```rs
use std::path::Path;

use page_validation::{SafetyLimits, validate_file};

let limits = SafetyLimits::default();
let report = validate_file(Path::new("input.pdf"), None, &limits)?;
println!("{report}");
# Ok::<(), page_validation::ValidationError>(())
```
