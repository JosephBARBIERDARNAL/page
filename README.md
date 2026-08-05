# page

`page` is an experimental PDF/A and PDF/UA validaton engine, written in Rust.

> [!WARNING]
> This project is experimental and isn't really usable yet.

<br>

## Installation

With Rust and Cargo installed:

```sh
cargo install --git https://github.com/pagestandards/page.git page_cli --bin page
```

`page` is installed to `~/.cargo/bin`; add that directory to your `PATH` if needed.

<br>

## Usage

```sh
page validate document.pdf
```

```bash
Profile : PDF/A-1b
Result  : Conformant

✓ PDF syntax
✓ PDF/A-1b
✓ Metadata
✓ Color
✓ Fonts
✓ Images
✓ Graphics
✓ Interactive content
✓ Structure

Time    : 0.005s
```

If the document does not declare a profile, `page` exits with an explicit error. Use `--profile` to select a profile instead:

```sh
page validate document.pdf --profile a-1b
```

```bash
Profile : PDF/A-1b
Result  : Non-conformant

✓ PDF syntax
✓ PDF/A-1b
✓ Metadata
✓ Color
✓ Fonts
✓ Images
✓ Graphics
✓ Interactive content
✗ Structure

Time    : 0.005s
```

<br>

Add `--format details` to emit details about the failure:

```sh
page validate document.pdf --format details
```

```sh
Profile : PDF/A-1b
Result  : Non-conformant

✗ PDF syntax
✓ PDF/A-1b
✗ Metadata
✓ Color
✓ Fonts
✓ Images
✓ Graphics
✓ Interactive content
✗ Structure

Time    : 0.001s

Checks: 123 passed, 11 failed, 134 total
Document: PDF 1.4, 1 page(s), 4 object(s)
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


> [.........] are just placeholders of the actual messages

<br>

Or use `--format json` to emit the details as JSON:

```sh
page validate document.pdf --format json
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

Explicit formats that conflict with `.json` or `.txt` are rejected. File output is uncolored and leaves stdout empty.

Human-readable output uses colors when writing to a terminal. Set the `NO_COLOR` environment variable or pass `--no-color` to disable them.

<br>

## Roadmap

> [!IMPORTANT]
> page implements a PDF/A-1 validator based on ISO 19005-1 and is extensively verified against veraPDF 1.30.2, with documented limitations around the impractical [maximum indirect-object boundary](https://github.com/veraPDF/veraPDF-validation-profiles/wiki/PDFA-Part-1-rules#rule-6112-7).

- [x] PDF/A-1b
- [x] PDF/A-1a
- [ ] PDF/A-2a
- [ ] PDF/A-2b
- [ ] PDF/A-2u
- [ ] PDF/A-3a
- [ ] PDF/A-3b
- [ ] PDF/A-3u
- [ ] PDF/A-4
- [ ] PDF/A-4e
- [ ] PDF/A-4f
- [ ] PDF/UA-1
- [ ] PDF/UA-2

<br>

## License

The original `page` source code is licensed under [MIT](LICENSE).

The `page_validation` crate bundles Adobe CMap Resources under the BSD 3-Clause license;
see the [third-party notices](crates/page_validation/THIRD_PARTY_NOTICES.md).
Binary distributions must include both license documents.
