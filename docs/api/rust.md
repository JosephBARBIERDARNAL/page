---
title: "Rust"
---

The [`page_validation`](https://crates.io/crates/page_validation) crate validates PDF files against a supported PDF/A or PDF/UA profile. This page contains most common usages, but you can find reference documentation at [docs.rs](https://docs.rs/page_validation/latest/page_validation/index.html).

## Installation

Either run:

```sh
cargo add page_validation
```

Or add the following to your `Cargo.toml`:

```toml
[dependencies]
page_validation = "0.6.1"
```

## Check compliance of a PDF

`is_pdf_compliant()` is the fastest way to get a simple true/false compliance result against a profile. It stops once it finds a failing rule and returns the boolean directly:

```rust
use std::path::Path;
use page_validation::{ValidationProfile, SafetyLimits, is_pdf_compliant};

let is_compliant = is_pdf_compliant(
    Path::new("file.pdf"),            // path to a PDF
    Some(ValidationProfile::PdfUA1),  // an optional profile
    &SafetyLimits::default(),         // see below
)?;
println!("{is_compliant}");
```

If the profile isn't specified, it reads the PDF/A or PDF/UA profile declared in the document's XMP metadata and returns `Result<bool, ValidationError>`. A missing, malformed, or unsupported profile declaration produces a `ValidationError`. Also see, [safety limits](#safety-limits).

!!! info

    If you want to run it on bytes instead of a file, use `is_pdf_compliant_bytes()`, which provides the same `Result<bool, ValidationError>` API but expects a `&[u8]` instead of a `&Path`.

## Validate a PDF with details

If you details about which rule failed, use `validate_pdf()`:

```rust
use std::path::Path;
use page_validation::{SafetyLimits, validate_pdf};

let doc = Path::new("document.pdf")
let report = validate_pdf(doc, None, &SafetyLimits::default())?;

if report.is_compliant {
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

!!! info

    If you want to run it on bytes instead of a file, use `validate_pdf_bytes()`, which provides the same API but expects a `&[u8]` instead of a `&Path`.

## Select a profile explicitly

Pass a profile to `validate_pdf()` when the caller, rather than the document, selects it:

```rust
use std::path::Path;
use page_validation::{SafetyLimits, ValidationProfile, validate_pdf};

let report = validate_pdf(
    Path::new("document.pdf"),
    Some(ValidationProfile::PdfA1b),
    &SafetyLimits::default(),
);
```

The explicit-profile call returns `Result<ValidationReport, ValidationError>`. Unlike profile inference, it does not require the document to contain a usable profile declaration. The declaration can still fail the selected profile's metadata rules.

## Failures

Each report contains a list of failures:

```rust
use std::path::Path;
use page_validation::{SafetyLimits, validate_pdf};

let report = validate_pdf(
    Path::new("file.pdf"),
    None,
    &SafetyLimits::default()
)?;

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

## Use the exit code

For command-line integrations or automated checks, the report can provide an appropriate process exit code:

```rust
std::process::exit(report.exit_code());
```
