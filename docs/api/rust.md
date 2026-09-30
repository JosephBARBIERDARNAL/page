---
title: "Rust"
---

The [`page_validation`](https://crates.io/crates/page_validation) crate validates PDF files against a supported PDF/A or PDF/UA profile. This page contains most common usages, but you can find reference documentation at [docs.rs](https://docs.rs/page_validation/latest/page_validation/index.html).

## Installation

```sh
cargo add page_validation
```

## Check compliance of a PDF

`is_pdf_compliant()` is the fastest way to get a simple true/false compliance result against a profile. It uses **lazy validation**: it stops once it finds a failing rule and returns the boolean directly:

```rust
use page_validation::{ValidationOptions, is_pdf_compliant};

let is_compliant = is_pdf_compliant("file.pdf", &ValidationOptions::default())?;
println!("{is_compliant}");
```

Every validation function takes a path (anything implementing `AsRef<Path>`) or bytes, plus a `&ValidationOptions`. `ValidationOptions::default()` infers the profile and uses the default [safety limits](#safety-limits); chain `.profile(...)` and `.limits(...)` to change them.

If the profile isn't specified, it reads the PDF/A or PDF/UA profile declared in the document's XMP metadata and returns `Result<bool, ValidationError>`. A missing, malformed, or unsupported profile declaration produces a `ValidationError`.

!!! info

    If you want to run it on bytes instead of a file, use `is_pdf_compliant_bytes()`, which provides the same `Result<bool, ValidationError>` API but expects a `&[u8]` instead of a `&Path`.

## Validate a PDF with details

If you need details about which rule failed, use `validate_pdf()`:

```rust
use page_validation::{ValidationOptions, validate_pdf};

let report = validate_pdf("document.pdf", &ValidationOptions::default())?;

if report.is_compliant {
    println!("The document passed all implemented rules.");
} else {
    println!("{} rules failed", report.rules.failed);
    println!("{} checks failed", report.checks.failed);
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

Set a profile in the options when the caller, rather than the document, selects it:

```rust
use page_validation::{ValidationOptions, ValidationProfile, validate_pdf};

let options = ValidationOptions::default().profile(ValidationProfile::PdfA1b);
let report = validate_pdf("document.pdf", &options);
```

The explicit-profile call returns `Result<ValidationReport, ValidationError>`. Unlike profile inference, it does not require the document to contain a usable profile declaration.

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

Safety limits protect the validator from excessively large or complex inputs. Defaults are sufficient for most cases:

```rust
use page_validation::SafetyLimits;

let limits = SafetyLimits {
    max_input_size: 256 * 1024 * 1024,                 // 256 MiB
    max_decoded_stream_size: 32 * 1024 * 1024,         // 32 MiB
    max_total_decoded_content_size: 256 * 1024 * 1024, // 256 MiB
    max_form_invocations: 10_000,                      // Form XObject expansions per document
    max_object_count: 1_000_000,                       // 1,000,000 objects
    max_reference_depth: 256,                          // 256 levels
    max_xref_revisions: 1_024,                         // 1,024 revisions
    max_table_span: 1_024,                             // rows or columns per cell
    max_table_grid_rows: 1_024,                        // rows
    max_table_grid_columns: 1_024,                     // columns
    max_table_grid_cells: 1_000_000,                   // cells
    max_unicode_cmap_mappings: 1_000_000,              // mappings per ToUnicode CMap
};
let report = validate_pdf("document.pdf", &ValidationOptions::default().limits(limits))?;
```

`max_decoded_stream_size` bounds one decoded stream and `max_total_decoded_content_size` bounds the combined decoded page, Form, appearance, Pattern, and Type3 content plus font streams retained by font inspection for one document. `max_form_invocations` bounds Form XObject expansions across all pages and nested content in one document. `max_xref_revisions` bounds the number of incremental-update revisions read from the cross-reference chain. `max_table_span` bounds the row or column span of an individual tagged-table cell. `max_table_grid_rows`, `max_table_grid_columns`, and `max_table_grid_cells` bound the derived table-grid dimensions and total cells. `max_unicode_cmap_mappings` bounds the total mappings expanded from one ToUnicode CMap.

For trusted files, pass `SafetyLimits::unlimited()` through the options:

```rust
let options = ValidationOptions::default().limits(SafetyLimits::unlimited());
let report = validate_pdf("document.pdf", &options)?;
```

The factory sets every configurable bound to its native integer maximum. You can restore individual bounds using struct update syntax. Validation may consume unrestricted memory and CPU; see the [safety limits guide](../guide/safety-limits.md).

## Use the exit code

For command-line integrations or automated checks, the report can provide an appropriate process exit code:

```rust
std::process::exit(report.exit_code());
```
