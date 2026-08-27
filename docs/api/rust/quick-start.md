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

let report = validate_file(Path::new("file.pdf"), None, &SafetyLimits::default())?;

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
- An `Optional` validation profile.
- A reference to [`SafetyLimits`](#safety-limits).

It reads the PDF/A or PDF/UA profile declared in the document's XMP metadata and returns `Result<ValidationReport, ValidationError>`. A missing, malformed, or unsupported profile declaration produces a `ValidationError`.

## Check compliance without collecting details

`is_pdf_compliant` uses the same validation logic as the CLI summary. It stops once it finds a failing rule and returns both the selected profile and the boolean result:

```rust
use std::path::Path;

use page_validation::{SafetyLimits, ValidationInput, is_pdf_compliant};

let result = is_pdf_compliant(
    ValidationInput::File(Path::new("file.pdf")),
    None,
    &SafetyLimits::default(),
)?;
println!("{}: {}", result.profile, result.is_compliant);
```

## Safety limits

The goal of the safety limits protect the validator from excessively large or complex inputs. Defaults are the following and should be sufficient for most cases:

```rust
use page_validation::SafetyLimits;

let limits = SafetyLimits {
    max_input_size: 100 * 1024 * 1024,                 // 100 MiB
    max_decoded_stream_size: 32 * 1024 * 1024,         // 32 MiB
    max_total_decoded_content_size: 100 * 1024 * 1024, // 100 MiB
    max_object_count: 500_000,                         // 500,000 objects
    max_reference_depth: 256,                          // 256 levels
};
```

`max_decoded_stream_size` bounds one decoded stream and `max_total_decoded_content_size` bounds the total decoded page, Form, appearance, Pattern, and Type3 content inspected for one document.

## Select a profile explicitly

Pass a profile to `validate_file` when the caller, rather than the document, selects it:

```rust
use std::path::Path;

use page_validation::{
    SafetyLimits, ValidationProfile, validate_file,
};

let report = validate_file(
    Path::new("document.pdf"),
    Some(ValidationProfile::PdfA1b),
    &SafetyLimits::default(),
);
```

The explicit-profile call returns `Result<ValidationReport, ValidationError>`. Unlike profile inference, it does not require the document to contain a usable profile declaration. The declaration can still fail the selected profile's metadata rules.

## Validate bytes

`validate_bytes` provides the same inferred and explicit behaviors for an in-memory PDF:

```rust
use page_validation::{
    SafetyLimits,
    ValidationProfile,
    validate_bytes,
};

let bytes = std::fs::read("document.pdf")?;
let inferred_report = validate_bytes(&bytes, None, &SafetyLimits::default())?;
let explicit_report = validate_bytes(
    &bytes,
    Some(ValidationProfile::PdfA1b),
    &SafetyLimits::default(),
);
```

## Failures

Each report contains a list of failures:

```rust
use std::path::Path;
use page_validation::{SafetyLimits, validate_file};

let report = validate_file(Path::new("file.pdf"), None, &SafetyLimits::default())?;

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
