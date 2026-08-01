---
title: "Rust crate"
---

# Using the `tag_validation` Crate

The `tag_validation` crate validates PDF files against a supported PDF/A or PDF/UA profile.

## Add the dependency

```toml
[dependencies]
tag_validation = "0.1.0"
```

## Validate a PDF

```rust
use std::path::Path;

use tag_validation::{SafetyLimits, ValidationProfile, validate_file};

fn main() {
    let report = validate_file(
        Path::new("document.pdf"),
        ValidationProfile::PdfA1b,
        &SafetyLimits::default(),
    );

    println!("{report}");

    if report.implemented_checks_passed {
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
}
```

`validate_file` takes:

- The path to the PDF.
- A `ValidationProfile`, such as `ValidationProfile::PdfA1b`.
- A reference to [`SafetyLimits`](#configure-safety-limits).

It returns a `ValidationReport` and prints something like this:

```
Preliminary PDF/A validation
Profile: PDF/A-1b
Result: failed
Checks: 124 passed, 10 failed, 134 total
Document: PDF 1.5, 13 page(s), 128 object(s)
```

## Configure safety limits

Safety limits protect the validator from excessively large or complex inputs.

```rust
use tag_validation::SafetyLimits;

let limits = SafetyLimits {
    max_input_size: 100 * 1024 * 1024,
    max_decoded_stream_size: 32 * 1024 * 1024,
    max_object_count: 500_000,
    max_reference_depth: 128,
};
```

Use `SafetyLimits::default()` when the built-in defaults are sufficient.

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
use tag_validation::FailureCategory;

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
