To limit resource use when processing untrusted PDFs, `page` enforces configurable bounds on input size, decoded data, parsed objects, reference depth, and selected inspection work.

| Option CLI                         |   Default | Purpose                                                                             |
| ---------------------------------- | --------: | ----------------------------------------------------------------------------------- |
| `--max-input-size`                 |   256 MiB | Maximum input file size.                                                            |
| `--max-decoded-stream-size`        |    32 MiB | Maximum decoded size of one stream.                                                 |
| `--max-total-decoded-content-size` |   256 MiB | Maximum combined decoded content and retained font-stream data.                     |
| `--max-form-invocations`          |    10,000 | Maximum Form XObject expansions across one document.                                |
| `--max-object-count`               | 1,000,000 | Maximum number of parsed indirect objects.                                          |
| `--max-reference-depth`            |       256 | Maximum reference-chain depth.                                                      |
| `--max-xref-revisions`             |     1,024 | Maximum number of incremental-update revisions read from the cross-reference chain. |
| `--max-table-span`                 |     1,024 | Maximum number of rows or columns covered by one table cell.                        |
| `--max-table-grid-rows`            |     1,024 | Maximum number of rows represented in an inspected table grid.                      |
| `--max-table-grid-columns`         |     1,024 | Maximum number of columns represented in an inspected table grid.                   |
| `--max-table-grid-cells`           | 1,000,000 | Maximum number of cells represented in an inspected table grid.                     |
| `--max-unicode-cmap-mappings`      | 1,000,000 | Maximum number of mappings expanded from one ToUnicode CMap.                        |

The defaults should be large enough for most files while bounding resource use. You can adjust individual limits or disable all of them for a validation call.

## Disable all safety limits

**Use unlimited limits only with trusted files.** Disabling these bounds allows validation to consume unrestricted memory and CPU, subject to the platform's capacity. Keep the default limits when processing untrusted PDFs.

In the CLI, pass `--disable-safety-limits`:

```sh
page document.pdf --disable-safety-limits
```

The flag cannot be combined with any explicit `--max-*` option, even if that option specifies its default value.

In Rust, Python, and JavaScript, pass the unlimited preset through the `limits` option:

=== "Rust"

    ```rust
    use page_validation::{SafetyLimits, ValidationOptions, validate_pdf};

    let options = ValidationOptions::default().limits(SafetyLimits::unlimited());
    let report = validate_pdf("document.pdf", &options)?;
    ```

=== "Python"

    ```python
    import page

    report = page.validate_pdf("document.pdf", limits=page.SafetyLimits.unlimited())
    ```

=== "JavaScript"

    ```js
    import { SafetyLimits, validatePdfBytes } from "page-validation-wasm";

    const report = await validatePdfBytes(bytes, { limits: SafetyLimits.unlimited() });
    ```

The same preset works with the bytes and compliance-only functions. Existing calls continue to use the default safety limits when no preset is supplied.

The preset sets all 12 configurable bounds to their native integer maxima. Rust and Python expose those integer values; JavaScript exposes `Infinity` and translates it to the target's native maxima in Wasm. Platform bounds, cycle detection, arithmetic checks, parser validity checks, and PDF conformance requirements remain active.

## Restore an individual limit

Each factory returns a fresh limits object. Library callers can start with unlimited limits and restore selected bounds:

=== "Rust"

    ```rust
    let limits = SafetyLimits::unlimited().with_max_input_size(512 * 1024 * 1024);
    ```

=== "Python"

    ```python
    limits = page.SafetyLimits.unlimited()
    limits.max_input_size = 512 * 1024 * 1024
    ```

=== "JavaScript"

    ```js
    const limits = SafetyLimits.unlimited();
    limits.maxInputSize = 512 * 1024 * 1024;
    ```
