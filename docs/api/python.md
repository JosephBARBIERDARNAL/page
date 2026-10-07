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

`is_pdf_compliant()` is the fastest way to get a simple true/false compliance result for a profile. It uses **lazy validation**: it stops once it finds a failing rule and returns the boolean directly:

=== "Validate file"

    ```python
    import page

    page.is_pdf_compliant("bench/health-of-canadians-2025.pdf", profile="ua1")
    ```

=== "Validate bytes"

    If you want to run it on an in-memory PDF, use `is_pdf_compliant_bytes()`. It accepts `bytes`, `bytearray`, and `memoryview` values:

    ```python
    import page
    from pathlib import Path

    data = bytearray(Path("document.pdf").read_bytes())

    page.is_pdf_compliant_bytes(data, profile="ua1")
    ```

## Validate a PDF with details

If you need details about **which rules failed**, use `validate_pdf()` instead:

=== "Validate file"

    ```python
    import page

    report = page.validate_pdf("document.pdf", profile="ua1")

    if report.is_compliant:
        print("The document passed all implemented rules.")
    else:
        print(f"{report.rules.failed} rules failed")
        print(f"{report.checks.failed} checks failed")
        for failure in report.failures:
            print(f"[{failure.rule_id}] {failure.message}")
    ```

=== "Validate bytes"

    If you want to run it on an in-memory PDF, use `validate_pdf_bytes()`. It accepts `bytes`, `bytearray`, and `memoryview` values:

    ```python
    import page
    from pathlib import Path

    data = memoryview(Path("document.pdf").read_bytes())

    report = page.validate_pdf_bytes(data, profile="ua1")
    ```

## Profile selection

If `profile` isn't specified, it reads the PDF/A or PDF/UA profile declared in the document's XMP metadata. Since this isn't the case for all PDFs, a document that doesn't declare it will raise an error.

```python
import page

page.validate_pdf("document.pdf")
```

Profile can either be a string or a `ValidationProfile`:

```python
import page

# Those are equivalents
page.validate_pdf("document.pdf", profile="ua1")
page.validate_pdf("document.pdf", profile=page.ValidationProfile.PDF_UA_1)
```

You can find all available profiles with:

```python
list(page.ValidationProfile)
```

```python
[<ValidationProfile.PDF_A_1B: '1b'>,
<ValidationProfile.PDF_A_1A: '1a'>,
<ValidationProfile.PDF_A_2B: '2b'>,
<ValidationProfile.PDF_A_2A: '2a'>,
<ValidationProfile.PDF_A_2U: '2u'>,
<ValidationProfile.PDF_A_3B: '3b'>,
<ValidationProfile.PDF_A_3A: '3a'>,
<ValidationProfile.PDF_A_3U: '3u'>,
<ValidationProfile.PDF_UA_1: 'ua1'>]
```

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

Parser, profile, and safety-limit failures raise their specific `page.PageError` subclasses.

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

For **trusted files**, pass `page.SafetyLimits.unlimited()` through the existing limits argument:

```python
import page

report = page.validate_pdf("document.pdf", limits=page.SafetyLimits.unlimited())
```

The factory returns a fresh object with every configurable bound set to its native integer maximum. Validation may consume unrestricted memory and CPU; see the [safety limits guide](../guide/safety-limits.md) for details about each limits.

## Export the report

Validation reports can be exported as JSON:

```python
import page

report = page.validate_pdf("document.pdf")

with open("report.json", "w") as output:
    output.write(report.to_json())
```

Use `report.to_dict()` to get the same report structure as a Python dictionary:

```python
report_data = report.to_dict()
print(report_data["is_compliant"])
print(report_data["failures"])
```

Each JSON failure includes `category` (`metadata` or `conformance`) and `object_id`. The object ID contains `object_number` and `generation` when the finding is attributed to an indirect object, and is `null` otherwise.
