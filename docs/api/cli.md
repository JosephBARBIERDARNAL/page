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

Add `--format details` to emit each failure's rule ID, message, category, and known object locations. Page numbers are one-based and identify pages, page-owned annotations and content streams, and used resources and their dependencies. Shared objects can list several pages; document-wide objects and objects without an established page association have no page numbers. The labels before the colons are bold in an interactive terminal, and a blank line separates each rule:

```sh
page document.pdf --profile 1b --format details
```

```
Result  : Non-conformant
Rules   : 2 failed rules / 134 total
Profile : PDF/A-1b
Time    : 0.017s

[ISO 19005-1:2005:6.2.3.2:1] Conformance: ICCBased profile has class "mntr", colour space "GRAY", and version 4.2
  Location: object 12 0 R

[ISO 19005-1:2005:6.7.11:1] Metadata: XMP does not contain the PDF/A Identification schema
```

Findings with multiple locations show the total number of distinct objects and up to three locations. Their description comes from the first finding for that rule:

```text
[ISO 14289-1:2014:7.18.1:2] Conformance: annotation 0 on page 4 has neither a non-empty /Contents entry nor a non-empty /Alt entry in its enclosing structure element
  Locations: 488 objects
  Location: object 11 0 R
  Location: object 12 0 R
  Location: object 13 0 R
  … 485 more locations (use --locations all)
```

Add `--locations all` to include every location in detailed terminal or file output. JSON always includes every known location:

```sh
page document.pdf --profile ua1 --format details --locations all
page document.pdf --profile ua1 --format details --locations all --output report.txt
```

<br>

Or use `--format json` to emit the details as JSON. Each failure includes its rule ID, message, category, and `locations`. Each location contains `object_number`, `generation`, and `page_numbers`; empty lists indicate that the corresponding location is unavailable. Locations refer to all findings for the rule, while the message retains the first description:

```sh
page document.pdf --profile 1b --format json
```

```json
{
  "source": "document.pdf",
  "profile": "1b",
  "is_compliant": false,
  "rules": {
    "total": 134,
    "passed": 132,
    "failed": 2
  },
  "failures": [
    {
      "rule_id": "ISO 19005-1:2005:6.2.3.2:1",
      "message": "ICCBased profile has class \"mntr\", colour space \"GRAY\", and version 4.2",
      "category": "conformance",
      "locations": [{"object_number": 12, "generation": 0, "page_numbers": []}]
    },
    {
      "rule_id": "ISO 19005-1:2005:6.7.11:1",
      "message": "XMP does not contain the PDF/A Identification schema",
      "category": "metadata",
      "locations": []
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

## Exit codes

| Code | Meaning                                                                                                                                       |
| ---: | --------------------------------------------------------------------------------------------------------------------------------------------- |
|  `0` | Validation completed with no failed checks, or `--help` / `--version` was requested.                                                          |
|  `1` | Operational failure, including I/O or safety-limit errors, missing or invalid profile declarations, output errors, and invalid CLI arguments. |
|  `2` | The PDF is non-compliant, or the PDF parser could not parse the document.                                                                     |

## Safety limits

The CLI exposes every `SafetyLimits` bound. Defaults are suitable for most documents and can be overridden per invocation:

```sh
page document.pdf --max-table-span 256 --max-table-grid-cells 250000
```

| Option                             |   Default | Purpose                                                                             |
| ---------------------------------- | --------: | ----------------------------------------------------------------------------------- |
| `--max-input-size`                 |   256 MiB | Maximum input file size.                                                            |
| `--max-decoded-stream-size`        |    32 MiB | Maximum decoded size of one stream.                                                 |
| `--max-total-decoded-content-size` |   256 MiB | Maximum combined decoded content and retained font-stream data.                     |
| `--max-form-invocations`           |    10,000 | Maximum Form XObject expansions across one document.                                |
| `--max-object-count`               | 1,000,000 | Maximum number of parsed indirect objects.                                          |
| `--max-reference-depth`            |       256 | Maximum reference-chain depth.                                                      |
| `--max-xref-revisions`             |     1,024 | Maximum number of incremental-update revisions read from the cross-reference chain. |
| `--max-table-span`                 |     1,024 | Maximum number of rows or columns covered by one table cell.                        |
| `--max-table-grid-rows`            |     1,024 | Maximum number of rows represented in an inspected table grid.                      |
| `--max-table-grid-columns`         |     1,024 | Maximum number of columns represented in an inspected table grid.                   |
| `--max-table-grid-cells`           | 1,000,000 | Maximum number of cells represented in an inspected table grid.                     |
| `--max-unicode-cmap-mappings`      | 1,000,000 | Maximum number of mappings expanded from one ToUnicode CMap.                        |

For trusted files, disable all configurable limits with `page document.pdf --disable-safety-limits`. The flag conflicts with any explicitly supplied `--max-*` option. Validation may consume unrestricted memory and CPU; see the [safety limits guide](../guide/safety-limits.md) for details.

## Colors

By default, `page` uses color in the terminal output.

![Example of terminal output, with colors on some key words.](../../images/terminal-colors.png)

In order to follow the [NO_COLOR standard](https://no-color.org/), you can either set the `NO_COLOR` environment variable to 1 or pass `--color never` to disable them. Other options are `--color auto` (default) and `--color always`.
