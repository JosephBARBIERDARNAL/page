---
title: "Rust"
---

# Using the `page_validation` Crate

The `page_validation` crate validates PDF files against a supported PDF/A or PDF/UA profile.

## Add the dependency

Run `cargo add page_validation`, or add the following to your `Cargo.toml`:

```toml
[dependencies]
page_validation = "0.4.0"
```

## Validate a PDF

```rust
use std::path::Path;

use page_validation::{SafetyLimits, validate_file};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = validate_file(Path::new("document.pdf"), &SafetyLimits::default())?;

    println!("{report}");

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

    Ok(())
}
```

`validate_file` takes:

- The path to the PDF.
- A reference to [`SafetyLimits`](#configure-safety-limits).

It reads the PDF/A or PDF/UA profile declared in the document's XMP metadata and returns `Result<ValidationReport, ValidationError>`. A missing, malformed, or unsupported profile declaration produces a `ValidationError`. Printing a successful report produces output like this:

```
Preliminary PDF/A validation
Profile: PDF/A-1b
Result: failed
Checks: 124 passed, 10 failed, 134 total
Document: PDF 1.5, 13 page(s), 128 object(s)
```

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

The explicit-profile function returns a `ValidationReport` directly. Unlike
profile inference, it does not require the document to contain a usable profile
declaration. The declaration can still fail the selected profile's metadata
rules.

## Validate bytes

`validate_bytes` and `validate_bytes_with_profile` provide the same inferred and
explicit behaviors for an in-memory PDF:

```rust
use page_validation::{
    SafetyLimits, ValidationProfile, validate_bytes, validate_bytes_with_profile,
};

let bytes = std::fs::read("document.pdf")?;
let inferred_report = validate_bytes(&bytes, &SafetyLimits::default())?;
let explicit_report = validate_bytes_with_profile(
    &bytes,
    ValidationProfile::PdfA1b,
    &SafetyLimits::default(),
);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Configure safety limits

Safety limits protect the validator from excessively large or complex inputs.

```rust
use page_validation::SafetyLimits;

let limits = SafetyLimits {
    max_input_size: 100 * 1024 * 1024,
    max_decoded_stream_size: 32 * 1024 * 1024,
    max_total_decoded_content_size: 100 * 1024 * 1024,
    max_object_count: 500_000,
    max_reference_depth: 256,
};
```

Use `SafetyLimits::default()` when the built-in defaults are sufficient.

`max_decoded_stream_size` bounds one decoded stream. `max_total_decoded_content_size` bounds the total decoded page, Form, appearance, Pattern, and Type3 content inspected for one document.

## Inspect failures

Each report contains a list of failures:

```rust
for failure in &report.failures {
    println!("Rule: {}", failure.rule_id);
    println!("Category: {:?}", failure.category);
    println!("Message: {}", failure.message);
}
```

Failure categories distinguish conformance problems from parser or operational errors:

```rust
use page_validation::FailureCategory;

match failure.category {
    FailureCategory::Metadata | FailureCategory::Conformance => {
        // The PDF was parsed, but failed a validation rule.
    }
    FailureCategory::Parser => {
        // The PDF could not be parsed correctly.
    }
    FailureCategory::Operational => {
        // Validation failed because of I/O or another runtime issue.
    }
}
```

## Use the exit code

For command-line integrations or automated checks, the report can provide an appropriate process exit code:

```rust
std::process::exit(report.exit_code());
```
