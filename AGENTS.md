# Repository Guidelines

## Overview

tag provides an alternative to veraPDF written in Rust. veraPDF is the source of truth for the expected output of a given validation. ALWAYS use verapdf to confirm or infirm the output of a given test or rule. We're currently focusing on PDF/A-1b validation.

## Notes

- there is NO backward compatibility policy, breaking changes are allowed.
- when you found a very unexpected result in veraPDF, figure out whether it's a real upstream bug or not. In order to prove that it is one, you need to create a reprex (as minimalist as possible).

## Project Structure & Module Organization

This Rust 2024 project is a virtual Cargo workspace with two packages. Keep reusable PDF parsing, normalization, validation rules, reports, safety limits, and veraPDF differential logic in `crates/tag_validation`. Keep argument parsing, presentation, exit behavior, and executable entry points in `crates/tag_cli`. Keep internal validation logic separate from the CLI. The CLI may depend on the validation crate; the validation crate must never depend on the CLI crate or on client-only dependencies such as Clap.

Validation unit and integration tests live with `tag_validation`; keep its shared helpers in `crates/tag_validation/tests/common/`, PDF inputs and the differential manifest in `crates/tag_validation/tests/fixtures/`, and sanitized veraPDF output in `crates/tag_validation/tests/reference-reports/`. CLI contract tests live in `crates/tag_cli/tests/`. Build artifacts under `target/` are not source files.

## Testing Guidelines

Place focused unit tests beside their owning validation modules in `#[cfg(test)]` blocks and validation integration behavior in `crates/tag_validation/tests/*.rs`. Test argument parsing, output formats, and exit-status behavior in `crates/tag_cli/tests/*.rs`. Name tests for observable behavior, for example `rejects_encrypted_pdf`. Add regression coverage to the package that owns the behavior. There is no stated numeric coverage target. PDF fixtures are binary and hash-pinned by `crates/tag_validation/tests/fixture_integrity.rs`; update fixtures and their expected hashes intentionally.

## veraPDF Validation Rule Pages

Always follow official verapdf rules specs.

- **PDF/A-1 (ISO 19005-1)**
  - PDF/A-1a
  - PDF/A-1b
  - Rules:
    - https://github.com/veraPDF/veraPDF-validation-profiles/wiki/PDFA-Part-1-rules

- **PDF/A-2 (ISO 19005-2)**
  - PDF/A-2a
  - PDF/A-2b
  - PDF/A-2u
  - Rules (shared with PDF/A-3):
    - https://github.com/veraPDF/veraPDF-validation-profiles/wiki/PDFA-Parts-2-and-3-rules

- **PDF/A-3 (ISO 19005-3)**
  - PDF/A-3a
  - PDF/A-3b
  - PDF/A-3u
  - Rules (shared with PDF/A-2):
    - https://github.com/veraPDF/veraPDF-validation-profiles/wiki/PDFA-Parts-2-and-3-rules

- **PDF/A-4 (ISO 19005-4)**
  - PDF/A-4
  - PDF/A-4e
  - PDF/A-4f
  - Rules:
    - https://github.com/veraPDF/veraPDF-validation-profiles/wiki/PDFA-Part-4-rules

- **PDF/UA-1 (ISO 14289-1)**
  - Rules:
    - https://github.com/veraPDF/veraPDF-validation-profiles/wiki/PDFUA-Part-1-rules

- **PDF/UA-2 (ISO 14289-2)**
  - Rules:
    - https://github.com/veraPDF/veraPDF-validation-profiles/wiki/PDFUA-Part-2-rules

## Architecture

```text
`tag_cli`
    -> argument and output handling
    -> `tag_validation`
       -> bounded file input
       -> strict lopdf parser
       -> normalized PdfDocument model
          (metadata, XMP declaration, output intents, fonts)
       -> private bounded font, colour-space, graphics, annotation, action, and form inspections
       -> preliminary rule evaluator
       -> deterministic ValidationReport
```

Operational and parser failures are kept separate from metadata and conformance failures. Limits are configurable for input bytes, decoded stream bytes, object count, and reference-chain depth. Operational failures use `INPUT-IO-001` or `RESOURCE-LIMIT-001` and do not describe PDF conformance. Library tests and fixtures live under `crates/tag_validation/tests`; CLI contract tests live under `crates/tag_cli/tests`. Each package declares only the dependencies it uses.

## Differential testing against veraPDF

The `verapdf-diff` binary compares the local subset with an explicitly pinned
veraPDF installation:

```bash
cargo run -p tag_cli --bin verapdf-diff -- \
  --verapdf /path/to/verapdf \
  --expected-version 1.28.2 \
  --profile 1b \
  --format text \
  file.pdf another.pdf
```

The reference is veraPDF `1.28.2`, flavour `1b`. The runner first verifies the executable's version, then invokes each PDF separately with `--loglevel 0`, `--format json`, and `--flavour 1b`. Disabling veraPDF logging is necessary because Java warning records can otherwise be inserted into its JSON stdout. All process arguments are passed directly with `std::process::Command`; no shell command string is constructed.

The classifications are:

- `agreement`: veraPDF reports compliant and all local implemented checks pass.
- `both_noncompliant`: both validators reject the input or report failures.
- `coverage_gap`: the local subset passes while veraPDF reports failures from
  rules not implemented locally. This is expected during development and must
  never be read as a conformance result.
- `local_false_negative`: veraPDF passes while a local implemented check fails.
- `local_parser_discrepancy`: the local parser rejects a PDF that veraPDF can
  process.
- `reference_parser_discrepancy`: veraPDF cannot process a PDF that the local
  parser can process.
- `operational`: the executable is unavailable, its version is wrong, it times
  out, its report is invalid, or local input/resource handling fails.

`agreement`, `both_noncompliant`, and `coverage_gap` exit with status `0`. Semantic or parser discrepancies exit with status `2`; operational failures exit with status `1`. Across multiple files, operational status takes precedence over discrepancy status.

### Pinned rule mapping

The machine-readable source of truth for the PDF/A-1b rule mapping and coverage evidence is `crates/tag_validation/tests/fixtures/pdfa-1b-coverage.json`. The pinned veraPDF profile is `crates/tag_validation/tests/fixtures/PDFA-1B-1.28.xml`. See `docs/rules/pdfa-1b-rule-mapping.md` for the generated human-readable mapping.
