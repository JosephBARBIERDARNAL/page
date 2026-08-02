# page

`page` is an experimental PDF/A and PDF/UA validator, written in Rust.

> [!WARNING]
> This project is **very early stage**, and current focus is on the PDF/A-1b validation.

<br>

## Install

With Rust and Cargo installed:

```sh
cargo install --git https://github.com/page-pdf/page.git page_cli --bin page
```

`page` is installed to `~/.cargo/bin`; add that directory to your `PATH` if needed.

<br>

## Usage

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

Write the report to a file with `--output`. A `.json` extension selects JSON automatically; use `--format details` for a detailed text report:

```sh
page validate document.pdf --output report.json
page validate document.pdf --format details --output report.txt
```

Explicit formats that conflict with `.json` or `.txt` are rejected. Other extensions, including no extension, are allowed. File output is uncolored and leaves stdout empty.

Human-readable output uses colors when writing to a terminal. Set the `NO_COLOR` environment variable or pass `--no-color` to disable them.

<br>

## Roadmap

- [ ] PDF/A-1b. Missing implementations are:
  - complete annotations, actions, forms, and interactive-feature validation
- [ ] PDF/A-1a
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

## Performance

`page` is still in a preliminary stage, but current tests and benchmark I made (only on PDF/A-1b) give:

| Metric                   | `page` | `veraPDF` | ratio        |
| ------------------------ | ------ | --------- | ------------ |
| Validation time (median) | ~50 ms | ~1300 ms  | ~26× faster  |
| Peak RSS (median)        | ~13 MB | ~254 MB   | ~20× lighter |

Benchark code lives in [bench](./bench/).

<br>

## License

The original `page` source code is licensed under [MIT](LICENSE).

The `page_validation` crate bundles Adobe CMap Resources under the BSD 3-Clause license;
see the [third-party notices](crates/page_validation/THIRD_PARTY_NOTICES.md).
Binary distributions must include both license documents.
