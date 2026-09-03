---
title: "Python"
---

The `page-validation` package provides Python bindings for PDF/A and PDF/UA validation. This page contains the most common usages; see the [Python bindings repository](https://github.com/josephbarbierdarnal/page-validation) for reference documentation.

## Add the dependency

=== "uv"

      ```sh
      uv add page-validation
      ```

=== "pip"

      ```sh
      pip install page-validation
      ```

## Check compliance of a PDF

`is_pdf_compliant()` is the fastest way to get a simple true/false compliance result for a profile. It returns both the selected profile and the boolean result:

```python
import page

is_compliant: bool = page.is_pdf_compliant("file.pdf")
```

If the profile isn't specified, it reads the PDF/A or PDF/UA profile declared in the document's XMP metadata. A missing, malformed, or unsupported profile declaration, or an input that cannot be read or parsed, raises `page.ValidationError`.

!!! info

    If you want to run it on bytes instead of a file, use `is_pdf_compliant_bytes()`, which provides the same API but expects a `bytes` value instead of a path.

## Validate a PDF with details

If you need details about which rules failed, use `validate_pdf()`:

```python
import page

report = page.validate_pdf("document.pdf")

if report.is_compliant:
    print("The document passed all implemented checks.")
else:
    for failure in report.failures:
        print(f"[{failure.rule_id}] {failure.message}")
```

!!! info

    If you want to run it on bytes instead of a file, use `validate_pdf_bytes()`, which provides the same API but expects a `bytes` value instead of a path.

## Select a profile explicitly

Pass a profile to `validate_pdf()` when the caller, rather than the document, selects it:

```python
import page

report = page.validate_pdf(
    "document.pdf",
    page.ValidationProfile.PDF_A_1B,
)
```

The explicit-profile call does not require the document to contain a usable profile declaration. The declaration can still fail the selected profile's metadata rules. Use `is_pdf_compliant()` or the corresponding bytes function when you only need a boolean result.

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

Failure categories distinguish conformance problems from parser or operational errors:

```python
import page

for failure in report.failures:
    if failure.category in (
        page.FailureCategory.METADATA,
        page.FailureCategory.CONFORMANCE,
    ):
        # The PDF was parsed, but failed a validation rule.
        pass
    elif failure.category == page.FailureCategory.PARSER:
        # The PDF could not be parsed correctly.
        pass
    elif failure.category == page.FailureCategory.OPERATIONAL:
        # Validation failed because of I/O or another runtime issue.
        pass
```

## Safety limits

Safety limits protect the validator from excessively large or complex inputs. Defaults are sufficient for most cases:

```python
import page

limits = page.SafetyLimits(
    max_input_size=256 * 1024 * 1024,                 # 256 MiB
    max_decoded_stream_size=32 * 1024 * 1024,         # 32 MiB
    max_total_decoded_content_size=256 * 1024 * 1024, # 256 MiB
    max_object_count=1_000_000,                       # 1,000,000 objects
    max_reference_depth=256,                          # 256 levels
    max_xref_revisions=1_024,                         # 1,024 revisions
)

report = page.validate_pdf("document.pdf", limits=limits)
```

`max_decoded_stream_size` bounds one decoded stream and `max_total_decoded_content_size` bounds the total decoded page, Form, appearance, Pattern, and Type3 content inspected for one document. `max_xref_revisions` bounds the number of incremental-update revisions read from the cross-reference chain.

## Use the exit code

For command-line integrations or automated checks, a report can provide an appropriate process exit code:

```python
import sys
import page

report = page.validate_pdf("document.pdf")
sys.exit(report.exit_code())
```

The exit code is `0` for a compliant report, `2` for a noncompliant report, and `1` when the report contains an operational failure.

## Export the report

Validation reports can be exported as JSON:

```python
import page

report = page.validate_pdf("document.pdf")

with open("report.json", "w") as output:
    output.write(report.to_json())
```
