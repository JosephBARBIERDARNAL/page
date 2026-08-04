# Repository Guidelines

## Overview

page provides an alternative to veraPDF written in Rust. veraPDF is the source of truth for the expected output of a given validation. ALWAYS use verapdf to confirm or infirm the output of a given test or rule. We're currently focusing on PDF/A-1b validation.

## Miscellanous rules

- there is NO backward compatibility policy, breaking changes are allowed.
- when you found a very unexpected result in veraPDF, figure out whether it's a real upstream bug or not. In order to prove that it is one, you need to create a reprex (as minimalist as possible).
- always write markdown paragraph / bullet point on a single line. Only use linebreaks for new paragraph, an heading, new bullet point, etc.
- every time you implement a new rule or a new feature, think of the impact it will have on performance and look for low hanging fruit that would improve performance.
- make sure, when possible, to reuse code from different profiles when possible
- ignore git changes that you didn't create, it's just someone else working on the project simultaneously.
- always run format/lint (`just fmt && just lint`) before submitting your changes and make sure it's all green
- don't add things like #[allow(dead_code)], allow less strict clippy rules, etc. Always explicitely ask before doing so with precise reasons of why that would be relevant.
- always check for ways to reuse code
- minimize useless abstraction

## Project Structure & Module Organization

This Rust 2024 project is a virtual Cargo workspace with two packages. Keep reusable PDF parsing, normalization, validation rules, reports, safety limits, and veraPDF differential logic in `crates/page_validation`. Keep argument parsing, presentation, exit behavior, and executable entry points in `crates/page_cli`. Keep internal validation logic separate from the CLI. The CLI may depend on the validation crate; the validation crate must never depend on the CLI crate or on client-only dependencies such as Clap.

Validation unit and integration tests live with `page_validation`; keep its shared helpers in `crates/page_validation/tests/common/`, PDF inputs and the differential manifest in `crates/page_validation/tests/fixtures/`, and sanitized veraPDF output in `crates/page_validation/tests/reference-reports/`. CLI contract tests live in `crates/page_cli/tests/`. Build artifacts under `target/` are not source files.

## Testing Guidelines

Place focused unit tests beside their owning validation modules in `#[cfg(test)]` blocks and validation integration behavior in `crates/page_validation/tests/*.rs`. Test argument parsing, output formats, and exit-status behavior in `crates/page_cli/tests/*.rs`. Name tests for observable behavior, for example `rejects_encrypted_pdf`. Add regression coverage to the package that owns the behavior. There is no stated numeric coverage target. PDF fixtures are binary and hash-pinned by `crates/page_validation/tests/fixture_integrity.rs`; update fixtures and their expected hashes intentionally.

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
`page_cli`
    -> argument and output handling
    -> `page_validation`
       -> bounded file input
       -> strict lopdf parser
       -> normalized PdfDocument model
          (metadata, XMP declaration, output intents, fonts)
       -> private bounded font, colour-space, graphics, annotation, action, and form inspections
       -> preliminary rule evaluator
       -> deterministic ValidationReport
```

Operational and parser failures are kept separate from metadata and conformance failures. Limits are configurable for input bytes, decoded stream bytes, object count, and reference-chain depth. Operational failures use `INPUT-IO-001` or `RESOURCE-LIMIT-001` and do not describe PDF conformance. Library tests and fixtures live under `crates/page_validation/tests`; CLI contract tests live under `crates/page_cli/tests`. Each package declares only the dependencies it uses.

## Differential testing against veraPDF

Entire source code of the veraPDF-library lives in `veraPDF-library/`, and it's exactly the one for 1.30.2. It's excluded from git tracking.

The `verapdf-diff` binary compares the local subset with an explicitly pinned
veraPDF installation:

```bash
cargo run -p page_cli --bin verapdf-diff -- \
  --verapdf /path/to/verapdf \
  --expected-version 1.30.2 \
  --profile 1b \
  --format text \
  file.pdf another.pdf
```

The reference is veraPDF `1.30.2`, flavour `1b`. The runner first verifies the executable's version, then groups PDFs into bounded batches of 32 by default and invokes veraPDF with `--loglevel 0`, `--format json`, and `--flavour 1b`. Use `--batch-size` to tune the bound. Disabling veraPDF logging is necessary because Java warning records can otherwise be inserted into its JSON stdout. Every PDF path is passed as a separate argument directly through `std::process::Command`; no shell command string is constructed.

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

The machine-readable source of truth for the PDF/A-1b rule mapping and coverage evidence is `crates/page_validation/tests/fixtures/pdfa-1b-coverage.json`. The pinned veraPDF profile is `crates/page_validation/tests/fixtures/PDFA-1B-1.28.xml`. See `docs/rules/pdfa-1b-rule-mapping.md` for the generated human-readable mapping.
