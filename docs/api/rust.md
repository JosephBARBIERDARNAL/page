The [`page_validation`](https://crates.io/crates/page_validation) crate validates PDF files against a supported PDF/A or PDF/UA profile. This page contains the most common uses, but you can find reference documentation at [docs.rs](https://docs.rs/page_validation/latest/page_validation/index.html).

## Installation

```sh
cargo add page_validation
```

## Check compliance of a PDF

`is_pdf_compliant()` is the fastest way to get a simple true/false compliance result for a profile. It uses [lazy validation](../guide/lazy-validation.md): it stops once it finds a failing rule and returns the boolean directly:

=== "Validate file"

    ```rust
    use page_validation::{ValidationOptions, ValidationProfile, is_pdf_compliant};

    let options = ValidationOptions::default().profile(ValidationProfile::PdfUa1);
    is_pdf_compliant("bench/health-of-canadians-2025.pdf", &options)?;
    ```

=== "Validate bytes"

    If you want to run it on bytes in memory, use `is_pdf_compliant_bytes()`. It accepts a `&[u8]`:

    ```rust
    use page_validation::{ValidationOptions, ValidationProfile, is_pdf_compliant_bytes};
    use std::fs;

    let bytes = fs::read("document.pdf")?;
    let options = ValidationOptions::default().profile(ValidationProfile::PdfUa1);

    is_pdf_compliant_bytes(&bytes, &options)?;
    ```

## Validate a PDF with details

If you need details about **which rules failed**, use `validate_pdf()` instead:

=== "Validate file"

    ```rust
    use page_validation::{ValidationOptions, ValidationProfile, validate_pdf};

    let options = ValidationOptions::default().profile(ValidationProfile::PdfUa1);
    let report = validate_pdf("document.pdf", &options)?;

    if report.is_compliant {
        println!("The document passed all implemented rules.");
    } else {
        println!("{} rules failed", report.rules.failed);
        println!("{} checks failed", report.checks.failed);
        for failure in &report.failures {
            println!("[{}] {}", failure.rule_id, failure.message);
        }
    }
    ```

=== "Validate bytes"

    If you want to run it on bytes in memory, use `validate_pdf_bytes()`. It accepts a `&[u8]`:

    ```rust
    use page_validation::{ValidationOptions, ValidationProfile, validate_pdf_bytes};
    use std::fs;

    let bytes = fs::read("document.pdf")?;
    let options = ValidationOptions::default().profile(ValidationProfile::PdfUa1);

    let report = validate_pdf_bytes(&bytes, &options)?;
    ```

## Profile selection

If `profile` isn't specified, it reads the PDF/A or PDF/UA profile declared in the document's XMP metadata. Since this isn't the case for all PDFs, a document that doesn't declare it will return a `PageError`.

```rust
use page_validation::{ValidationOptions, validate_pdf};

let report = validate_pdf("document.pdf", &ValidationOptions::default())?;
```

The profile can be a `ValidationProfile`:

```rust
use page_validation::{ValidationOptions, ValidationProfile, validate_pdf};

let options = ValidationOptions::default().profile(ValidationProfile::PdfUa1);
let report = validate_pdf("document.pdf", &options)?;
```

You can find all available profiles with:

```rust
use page_validation::ValidationProfile;

[
    ValidationProfile::PdfA1b,
    ValidationProfile::PdfA1a,
    ValidationProfile::PdfA2b,
    ValidationProfile::PdfA2a,
    ValidationProfile::PdfA2u,
    ValidationProfile::PdfA3b,
    ValidationProfile::PdfA3a,
    ValidationProfile::PdfA3u,
    ValidationProfile::PdfUa1,
];
```

## Failures

Each report contains a list of failures:

```rust
use page_validation::{ValidationOptions, validate_pdf};

let report = validate_pdf("file.pdf", &ValidationOptions::default())?;

for failure in &report.failures {
    println!("Rule: {}", failure.rule_id);
    println!("Category: {:?}", failure.category);
    println!("Message: {}", failure.message);
}
```

`report.rules` contains the total, passed, and failed implemented rules. `report.checks.failed` counts every raw finding, so several objects failing the same rule contribute one failed rule and multiple failed checks.

Failure categories describe findings from rules that ran on a parsed document:

```rust
use page_validation::FailureCategory;

for failure in &report.failures {
    match failure.category {
        FailureCategory::Metadata => println!("metadata finding {}", failure.rule_id),
        FailureCategory::Conformance => println!("conformance finding {}", failure.rule_id),
        _ => println!("finding from a future category {}", failure.rule_id),
    }
}
```

Input, parser, profile, and safety-limit failures return their specific `PageError` and do not appear in a report. Use `PageError::kind()` to classify them as `InputIo`, `Parser`, `Profile`, or `SafetyLimit`, and `PageError::rule_id()` to get the associated rule identifier.

## Safety limits

Safety limits protect the validator from excessively large or complex inputs. Defaults are sufficient for most cases:

```rust
use page_validation::{SafetyLimits, ValidationOptions, validate_pdf};

let limits = SafetyLimits::default()
    .with_max_input_size(256 * 1024 * 1024) // 256 MiB
    .with_max_decoded_stream_size(32 * 1024 * 1024) // 32 MiB
    .with_max_total_decoded_content_size(256 * 1024 * 1024) // 256 MiB
    .with_max_form_invocations(10_000) // Form XObject expansions per document
    .with_max_object_count(1_000_000) // 1,000,000 objects
    .with_max_reference_depth(256) // 256 levels
    .with_max_xref_revisions(1_024) // 1,024 revisions
    .with_max_table_span(1_024) // rows or columns per cell
    .with_max_table_grid_rows(1_024) // rows
    .with_max_table_grid_columns(1_024) // columns
    .with_max_table_grid_cells(1_000_000) // cells
    .with_max_unicode_cmap_mappings(1_000_000); // mappings per ToUnicode CMap

let report = validate_pdf("document.pdf", &ValidationOptions::default().limits(limits))?;
```

For **trusted files**, pass `SafetyLimits::unlimited()` through the options:

```rust
use page_validation::{SafetyLimits, ValidationOptions, validate_pdf};

let options = ValidationOptions::default().limits(SafetyLimits::unlimited());
let report = validate_pdf("document.pdf", &options)?;
```

The factory sets every configurable bound to its native integer maximum. Validation may consume unrestricted memory and CPU; see the [safety limits guide](../guide/safety-limits.md) for details about each limit.

## Export the report

Validation reports can be exported as JSON. Add `serde_json` to your project to serialize the stable report structure:

```sh
cargo add serde_json
```

```rust
use page_validation::{ValidationOptions, validate_pdf};

let report = validate_pdf("document.pdf", &ValidationOptions::default())?;
let json = serde_json::to_string(&report.json_report())?;
```

Use `report.json_report()` to get the same report structure as a Rust value:

```rust
let report_data = report.json_report();
println!("{}", report_data.is_compliant);
println!("{}", report_data.failures.len());
```

Each JSON failure includes `category` (`metadata` or `conformance`) and `object_id`. The object ID contains `object_number` and `generation` when the finding is attributed to an indirect object, and is `null` otherwise.
