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

Failure categories describe findings from rules that ran on a parsed document:

```rust
use page_validation::FailureCategory;

for failure in &report.failures {
    match failure.category {
        FailureCategory::Metadata => { /* XMP or document-information finding. */ }
        FailureCategory::Conformance => { /* PDF/A or PDF/UA rule finding. */ }
        _ => {
            // Handle categories added by future releases.
        }
    }
}
```

Input, parser, profile, and safety-limit failures are returned as `Err(ValidationError)` and do not appear in a report. Handle them separately with `?`, `match`, or `map_err`.

## Safety limits

Safety limits protect the validator from excessively large or complex inputs. Defaults are sufficient for most cases:

```rust
use page_validation::{SafetyLimits, ValidationOptions, validate_pdf};

let limits = SafetyLimits::default();
let report = validate_pdf("document.pdf", &ValidationOptions::default().limits(limits))?;
```

Customize individual bounds with chainable setters or field assignment:

```rust
let limits = SafetyLimits::default()
    .max_input_size(512 * 1024 * 1024)
    .max_decoded_stream_size(64 * 1024 * 1024);
```

`max_decoded_stream_size` bounds one decoded stream and `max_total_decoded_content_size` bounds the combined decoded page, Form, appearance, Pattern, and Type3 content plus font streams retained by font inspection for one document. `max_form_invocations` bounds Form XObject expansions across all pages and nested content in one document. `max_xref_revisions` bounds the number of incremental-update revisions read from the cross-reference chain. `max_table_span` bounds the row or column span of an individual tagged-table cell. `max_table_grid_rows`, `max_table_grid_columns`, and `max_table_grid_cells` bound the derived table-grid dimensions and total cells. `max_unicode_cmap_mappings` bounds the total mappings expanded from one ToUnicode CMap.

For trusted files, pass `SafetyLimits::unlimited()` through the options:

```rust
let options = ValidationOptions::default().limits(SafetyLimits::unlimited());
let report = validate_pdf("document.pdf", &options)?;
```

The factory sets every configurable bound to its native integer maximum. You can restore individual bounds with setters, for example `SafetyLimits::unlimited().max_input_size(512 * 1024 * 1024)`. Validation may consume unrestricted memory and CPU; see the [safety limits guide](../guide/safety-limits.md).

## Check compliance

Use `is_compliant` to check whether the document passed all implemented checks. Validation errors remain `Err(ValidationError)` and mean that no complete report was produced:

```rust
let report = validate_pdf("file.pdf", &ValidationOptions::default())?;
if !report.is_compliant {
    eprintln!("The document failed one or more implemented checks.");
}
```
