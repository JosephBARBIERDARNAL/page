# `validate_file_with_profile`

**Function**

Reads a file from disk and validates it against an explicitly selected profile, ignoring any profile the document declares in its own XMP metadata.

This is the file-based counterpart of [`validate_bytes_with_profile`]: it never fails outright. An unreadable file, an oversized file, or any error [`validate_bytes_with_profile`] can absorb becomes an operational or parser [`ValidationFailure`](crate::ValidationFailure) inside the returned report instead of a thrown error. The returned report has its `source` set to `path`.

## Arguments

- `path` - The PDF file to read and validate.
- `profile` - The PDF/A or PDF/UA profile to validate against, regardless of what the document's own XMP declares.
- `limits` - The resource bounds enforced while reading, parsing, and inspecting the document.

## Returns

A [`ValidationReport`] whose `checks_passed` is `true` only when every implemented check for `profile` passed, with `source` set to `path` and `failures` explaining every recorded problem, including operational ones such as an unreadable file.

## Examples

```no_run
use std::path::Path;

use page_validation::{SafetyLimits, ValidationProfile, validate_file_with_profile};

let limits = SafetyLimits::default();
let report = validate_file_with_profile(Path::new("input.pdf"), ValidationProfile::PdfA1b, &limits);
println!("{report}");
```
