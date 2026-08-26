---
title: "Quick start"
---

# Using the `page_validation` crate

The `page_validation` crate validates PDF files against a supported PDF/A or PDF/UA profile.

## Add the dependency

Either run:

```sh
cargo add page_validation
```

Or add the following to your `Cargo.toml`:

```toml
[dependencies]
page_validation = "0.4.0"
```

## Validate a PDF

```rust
use std::path::Path;
use page_validation::{SafetyLimits, validate_file};

let report = validate_file(Path::new("file.pdf"), &SafetyLimits::default())?;

if report.checks_passed {
    println!("The document passed all implemented checks.");
} else {
    for failure in &report.failures {
        eprintln!(
            "[{}] {}",
            failure.rule_id,
            failure.message,
        );
    }
}
```

`validate_file` takes:

- The path to the PDF.
- A reference to [`SafetyLimits`](./safety-limits.md).

It reads the PDF/A or PDF/UA profile declared in the document's XMP metadata and returns `Result<ValidationReport, ValidationError>`. A missing, malformed, or unsupported profile declaration produces a `ValidationError`.

## Select a profile explicitly

Use `validate_file_with_profile` when the caller, rather than the document, selects the validation profile:

```rust
use std::path::Path;

use page_validation::{
    SafetyLimits, ValidationProfile, validate_file_with_profile,
};

let report = validate_file_with_profile(
    Path::new("document.pdf"),
    ValidationProfile::PdfA1b,
    &SafetyLimits::default(),
);
```

The explicit-profile function returns a `ValidationReport` directly. Unlike profile inference, it does not require the document to contain a usable profile declaration. The declaration can still fail the selected profile's metadata rules.

## Validate bytes

`validate_bytes` and `validate_bytes_with_profile` provide the same inferred and explicit behaviors for an in-memory PDF:

```rust
use page_validation::{
    SafetyLimits,
    ValidationProfile,
    validate_bytes,
    validate_bytes_with_profile,
};

let bytes = std::fs::read("document.pdf")?;
let inferred_report = validate_bytes(&bytes, &SafetyLimits::default())?;
let explicit_report = validate_bytes_with_profile(
    &bytes,
    ValidationProfile::PdfA1b,
    &SafetyLimits::default(),
);
```

For error handling, check out [faillures page](./faillures.md).
