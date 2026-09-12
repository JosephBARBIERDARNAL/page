---
title: "CLI"
---

!!! info

     Check out [the dedicated page](../installation.md) for install instructions.

<br>

Validate one PDF (by default against the profile declared in its XMP metadata):

```sh
page document.pdf
```

```
Result  : Conformant
Profile : PDF/A-1b
Time    : 0.005s
```

If the document does not declare a profile, `page` exits with an explicit error. Use `--profile` to select a profile instead:

```sh
page document.pdf --profile ua1
```

```
Result  : Non-conformant
Profile : PDF/UA-1
Time    : 0.005s
```

<br>

Add `--format details` to emit details about the failure:

```sh
page document.pdf --format details
```

```
Result  : Non-conformant
Profile : PDF/A-1b
Time    : 0.007s

[PDFA1B-HEADER-BINARY-COMMENT-001] Conformance: [.........]
[PDFA1B-HEX-STRING-CHARACTERS-001] Conformance: [.........]
[PDFA1B-ID-SCHEMA-001] Metadata: [.........]
[PDFA1B-INFO-AUTHOR-001] Metadata: [.........]
[PDFA1B-INFO-CREATOR-001] Metadata: [.........]
[PDFA1B-INFO-KEYWORDS-001] Metadata: [.........]
[PDFA1B-INFO-PRODUCER-001] Metadata: [.........]
[PDFA1B-INFO-SUBJECT-001] Metadata: [.........]
[PDFA1B-INFO-TITLE-001] Metadata: [.........]
[PDFA1B-METADATA-STRUCTURE-001] Metadata: [.........]
[PDFA1B-TRAILER-ID-001] Conformance: [.........]
```

!!! note

      [ . . . . . . . . . ] are just placeholders of the actual messages

<br>

Or use `--format json` to emit the details as JSON:

```sh
page document.pdf --format json
```

```json
{
  "file": "document.pdf",
  "profile": "1b",
  "valid": false,
  "failures": [
    {
      "rule": "PDFA1B-TRAILER-ID-001",
      "message": "the applicable document trailer does not contain an ID entry"
    },
    {
      "rule": "PDFA1B-STREAM-LZW-001",
      "message": "a parsed stream declares the forbidden LZWDecode filter"
    }
  ]
}
```

<br>

Write the report to a file with `--output`. A `.json` extension selects JSON automatically; use `--format details` for a detailed text report:

```sh
page document.pdf --output report.json
page document.pdf --format details --output report.txt
```

Explicit formats that conflict with `.json` or `.txt` are rejected. Other extensions, including no extension, are allowed. File output is uncolored and leaves stdout empty.

## Safety limits

The CLI exposes every `SafetyLimits` bound. Defaults are suitable for most documents and can be overridden per invocation:

```sh
page document.pdf --max-table-span 256 --max-table-grid-cells 250000
```

| Option | Default | Purpose |
| --- | ---: | --- |
| `--max-input-size` | 256 MiB | Maximum input file size. |
| `--max-decoded-stream-size` | 32 MiB | Maximum decoded size of one stream. |
| `--max-total-decoded-content-size` | 256 MiB | Maximum total decoded page, Form, appearance, Pattern, and Type3 content. |
| `--max-object-count` | 1,000,000 | Maximum number of parsed indirect objects. |
| `--max-reference-depth` | 256 | Maximum reference-chain depth. |
| `--max-xref-revisions` | 1,024 | Maximum number of incremental-update revisions read from the cross-reference chain. |
| `--max-table-span` | 1,024 | Maximum number of rows or columns covered by one table cell. |
| `--max-table-grid-rows` | 1,024 | Maximum number of rows represented in an inspected table grid. |
| `--max-table-grid-columns` | 1,024 | Maximum number of columns represented in an inspected table grid. |
| `--max-table-grid-cells` | 1,000,000 | Maximum number of cells represented in an inspected table grid. |

## Colors

By default, `page` uses color in the terminal output.

![Example of terminal output, with colors on some key words.](../../images/terminal-colors.png)

In order to follow the [NO_COLOR standard](https://no-color.org/), you can either set the `NO_COLOR` environment variable to 1 or pass `--no-color` to disable them.
