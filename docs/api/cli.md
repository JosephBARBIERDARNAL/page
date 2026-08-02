---
title: "CLI"
---

Validate one PDF (by default against the profile declared in its XMP metadata):

```sh
page validate document.pdf
```

If the document does not declare a profile, `page` exits with an explicit error. Use `--profile` to select a profile instead:

```sh
page validate document.pdf --profile a-1b
```

```sh
PDF/A-1b: 1/134 implemented checks failed
```

<br>

Add `--format details` to emit details about the failure:

```sh
page validate document.pdf --profile a-1b --format details
```

```sh
Preliminary PDF/A validation
Profile: PDF/A-1b
Result: failed
Checks: 133 passed, 1 failed, 134 total
Document: PDF 1.4, 1 page(s), 62 object(s)
[PDFA1B-TRAILER-ID-001]: the applicable document trailer does not contain an ID entry
```

<br>

Or use `--format json` to emit the details as JSON:

```sh
page validate document.pdf --profile a-1b --format json
```

```json
{
  "file": "document.pdf",
  "profile": "a-1b",
  "valid": false,
  "failures": [
    {
      "rule": "PDFA1B-TRAILER-ID-001",
      "message": "the applicable document trailer does not contain an ID entry"
    }
  ]
}
```

<br>

Write the report to a file with `--output`. A `.json` extension selects JSON automatically; use `--format details` for a detailed text report:

```sh
page validate document.pdf --output report.json
page validate document.pdf --format details --output report.txt
```

Explicit formats that conflict with `.json` or `.txt` are rejected. Other extensions, including no extension, are allowed. File output is uncolored and leaves stdout empty.

<br>

!!! note

      Human-readable output uses colors when writing to a terminal. Set the `NO_COLOR` environment variable or pass `--no-color` to disable them.
