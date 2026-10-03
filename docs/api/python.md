---
title: "Python"
---

The [`page-validation`](https://pypi.org/project/page-validation/) package provides Python bindings for page.

## Installation

=== "uv"

      ```sh
      uv add page-validation
      ```

=== "pip"

      ```sh
      pip install page-validation
      ```

## Check compliance of a PDF

`is_pdf_compliant()` is the fastest way to get a simple true/false compliance result for a profile. It uses lazy validation: it stops once it finds a failing rule and returns the boolean directly:

```python
import page

is_compliant: bool = page.is_pdf_compliant("file.pdf")
```

Every validation function takes a path or bytes as its only positional argument; `profile` and `limits` are keyword-only. If the profile isn't specified, it reads the PDF/A or PDF/UA profile declared in the document's XMP metadata. Parsing failures raise `page.ParseError`, resource-limit failures raise `page.SafetyLimitError`, and missing, malformed, or unsupported profile declarations raise `page.ProfileError`; all three inherit from `page.ValidationError`. File read failures raise `OSError`, including `FileNotFoundError` for missing paths.

!!! info

    If you want to run it on bytes instead of a file, use `is_pdf_compliant_bytes()`, which provides the same API but expects a `bytes` value instead of a path.

## Validate a PDF with details

If you need details about which rules failed, use `validate_pdf()`:

```python
import page

report = page.validate_pdf("document.pdf")

print(report.source)

if report.is_compliant:
    print("The document passed all implemented rules.")
else:
    print(f"{report.rules.failed} rules failed")
    print(f"{report.checks.failed} checks failed")
    for failure in report.failures:
        print(f"[{failure.rule_id}] {failure.message}")
```

!!! info

    If you want to run it on bytes instead of a file, use `validate_pdf_bytes()`, which provides the same API but expects a `bytes` value instead of a path.

`report.source` contains the validated file path as a string. Reports from `validate_pdf_bytes()` have `source` set to `None`.

## Select a profile explicitly

Pass a profile to `validate_pdf()` when the caller, rather than the document, selects it:

```python
import page

report = page.validate_pdf(
    "document.pdf",
    profile=page.ValidationProfile.PDF_A_1B
)
```

The explicit-profile call does not require the document to contain a usable profile declaration. The declaration can still fail the selected profile's metadata rules. Use `is_pdf_compliant()` or the corresponding bytes function when you only need a boolean result.

`ValidationProfile` and `FailureCategory` are standard Python enums, so they can be iterated and expose `.name` and `.value`. `ValidationProfile.value` is the compact profile string, and `.is_implemented` reports whether page implements validation for that profile. Validation functions also accept a profile string directly, such as `profile="1b"`.

## Failures

Each report contains a list of failures:

```python
import page

report = page.validate_pdf("file.pdf")

for failure in report.failures:
    print(f"Rule: {failure.rule_id}")
    print(f"Category: {failure.category}")
    print(f"Message: {failure.message}")
```

`report.rules` contains the total, passed, and failed implemented rules. `report.checks.failed` counts every raw finding, so several objects failing the same rule contribute one failed rule and multiple failed checks.

Failure categories describe findings from rules that ran on a parsed document:

```python
import page

for failure in report.failures:
    if failure.category == page.FailureCategory.METADATA:
        print("metadata finding", failure.rule_id)
    elif failure.category == page.FailureCategory.CONFORMANCE:
        print("conformance finding", failure.rule_id)
```

Parser, profile, and safety-limit failures raise their specific `page.ValidationError` subclasses and do not appear in a report. File read failures use Python's `OSError` hierarchy.

## Safety limits

Safety limits protect the validator from excessively large or complex inputs. Defaults are sufficient for most cases:

```python
import page

limits = page.SafetyLimits(
    max_input_size=256 * 1024 * 1024,                 # 256 MiB
    max_decoded_stream_size=32 * 1024 * 1024,         # 32 MiB
    max_total_decoded_content_size=256 * 1024 * 1024, # 256 MiB
    max_form_invocations=10_000,                      # Form XObject expansions per document
    max_object_count=1_000_000,                       # 1,000,000 objects
    max_reference_depth=256,                          # 256 levels
    max_xref_revisions=1_024,                         # 1,024 revisions
    max_table_span=1_024,                             # rows or columns per cell
    max_table_grid_rows=1_024,                        # rows
    max_table_grid_columns=1_024,                     # columns
    max_table_grid_cells=1_000_000,                   # cells
    max_unicode_cmap_mappings=1_000_000,              # mappings per ToUnicode CMap
)

report = page.validate_pdf("document.pdf", limits=limits)
```

`max_decoded_stream_size` bounds one decoded stream and `max_total_decoded_content_size` bounds the combined decoded page, Form, appearance, Pattern, and Type3 content plus font streams retained by font inspection for one document. `max_form_invocations` bounds Form XObject expansions across all pages and nested content in one document. `max_xref_revisions` bounds the number of incremental-update revisions read from the cross-reference chain. `max_table_span` bounds the row or column span of an individual tagged-table cell. `max_table_grid_rows`, `max_table_grid_columns`, and `max_table_grid_cells` bound the derived table-grid dimensions and total cells. `max_unicode_cmap_mappings` bounds the total mappings expanded from one ToUnicode CMap.

For trusted files, pass `page.SafetyLimits.unlimited()` through the existing limits argument:

```python
report = page.validate_pdf("document.pdf", limits=page.SafetyLimits.unlimited())
```

The factory returns a fresh object with every configurable bound set to its native integer maximum. You can restore individual bounds by assigning to its fields. Validation may consume unrestricted memory and CPU; see the [safety limits guide](../guide/safety-limits.md).

## Check compliance

Use `is_compliant` to check whether the document passed all implemented checks:

```python
import page

report = page.validate_pdf("document.pdf")
if not report.is_compliant:
    print("The document failed one or more implemented checks.")
```

Parser, profile, and safety-limit failures raise their specific `page.ValidationError` subclasses before a report is returned. File read failures use Python's `OSError` hierarchy.

## Export the report

Validation reports can be exported as JSON:

```python
import page

report = page.validate_pdf("document.pdf")

with open("report.json", "w") as output:
    output.write(report.to_json())
```

Each JSON failure includes `category` (`metadata` or `conformance`) and `object_id`. The object ID contains `object_number` and `generation` when the finding is attributed to an indirect object, and is `null` otherwise.
