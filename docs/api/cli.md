---
title: "CLI"
---

Validate one PDF against a profile:

```sh
page document.pdf --profile a-1b
```

```sh
PDF/A-1b: 1/134 implemented checks failed
```

<br>

Add `--format details` to emit details about the failure:

```sh
page document.pdf --profile a-1b --format details
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
page document.pdf --profile a-1b --format json
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

!!! note

      Human-readable output uses colors when writing to a terminal. Set the `NO_COLOR` environment variable or pass `--no-color` to disable them.
