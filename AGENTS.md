# Repository Guidelines

## Overview

page is a new, modern, fast and lightweight PDF accessibility and compliance checker. It provides an alternative to veraPDF written in Rust.

## Miscellanous rules

- when you found a very unexpected result in veraPDF, figure out whether it's a real upstream bug or not. In order to prove that it is one, you need to create a reprex (as minimalist as possible).
- always write markdown paragraph / bullet point on a single line. Only use linebreaks for new paragraph, an heading, new bullet point, etc.
- Every Rust source file under `crates/page_validation/src/`, including newly added files, must start with a `//!` overview explaining its purpose, overall responsibility, and how it works, in at most two paragraphs. Keep the overview accurate when changing the file's responsibilities.
- every time you implement a new rule or a new feature, think of the impact it will have on performance and look for low hanging fruit that would improve performance.
- make sure, when possible, to reuse code from different profiles
- Treat every git change you did not explicitly make as belonging to someone else working on the project simultaneously; preserve it exactly and do not revert, restore, overwrite, format, or otherwise “clean up” that file.
- If a test, formatter, generator, or other command unexpectedly changes a tracked file, do not assume the command owns the change and do not restore it from `HEAD`; preserve the change and report it or ask before touching it.
- never add things like #[allow(dead_code)], allow less strict clippy rules, etc. Always explicitely ask before doing so with precise reasons of why that would be relevant, but this behavior should be banned by default.
- always check for ways to reuse code
- minimize useless abstraction
- when making a fix or adding a new feature, add a new entry in `docs/changelog.md` in the Dev section, matching the same format as other entries. Most changelog entries must have a github issue referenced, if you don't have one, ask for it.
- after making code changes, make sure the code is well formatted, linted and that unit tests pass. See @justfile for key commands to run.

## Project Structure & Module Organization

This Rust 2024 project is a virtual Cargo workspace with 4 packages:

- page_validation: core crate with the most code and the validation engine
- page_cli: CLI for the engine
- page_python: Python bindings with Pyo3
- page_wasm: Wasm bindings

Keep reusable PDF parsing, normalization, validation rules, reports, and safety limits in `crates/page_validation`. Keep CLI argument parsing, presentation, exit behavior, and executable entry points in `crates/page_cli`. Keep internal validation logic separate from the CLI. The CLI may depend on the validation crate; the validation crate must never depend on the CLI crate or on client-only dependencies such as Clap.

Validation unit and integration tests live with `page_validation`; keep its shared helpers in `crates/page_validation/tests/common/` and PDF inputs in `crates/page_validation/tests/fixtures/`. CLI contract tests live in `crates/page_cli/tests/`. Build artifacts under `target/` are not source files.

## Performance and benchmark

Performances are measured against verapdf in bench/benchmark.py, with 5 different documents and 3 different PDF profiles. Running the benchmark takes a lot of time (between 5 to 10 minutes) and shouldn't be run regularly.

## Security

By default, page enforces ressource limitations called "safety limits". See @docs/guide/safety-limits.md.

## Versionning

page uses semantic versionning. All crates must share the exact same version. Current version is 0.10.0.

## Testing Guidelines

Place focused unit tests beside their owning validation modules in `#[cfg(test)]` blocks and validation integration behavior in `crates/page_validation/tests/*.rs`. Test argument parsing, output formats, and exit-status behavior in `crates/page_cli/tests/*.rs`. Name tests for observable behavior, for example `rejects_encrypted_pdf`. Add regression coverage to the package that owns the behavior. There is no stated numeric coverage target. PDF fixtures are binary and hash-pinned by `crates/page_validation/tests/fixture_integrity.rs`; update fixtures and their expected hashes intentionally.

## Architecture

```text
-> bounded file input
-> strict lopdf parser
-> normalized PdfDocument model (metadata, XMP declaration, output intents, fonts)
-> private bounded font, colour-space, graphics, annotation, action, and form inspections
-> rule evaluator
-> deterministic ValidationReport
```

Operational and parser failures are kept separate from metadata and conformance failures. Limits are configurable for input bytes, decoded stream bytes, object count, and reference-chain depth. Operational failures use `INPUT-IO-001` or `RESOURCE-LIMIT-001` and do not describe PDF conformance. Library tests and fixtures live under `crates/page_validation/tests`; CLI contract tests live under `crates/page_cli/tests`. Each package declares only the dependencies it uses.

## veraPDF corpus conformance gate

The required pull-request CI check runs `page corpus` against the pinned `staging` revision `49de56cd987929932c9e4fbbbe67d052bf44ef83` of the external [veraPDF corpus](https://github.com/veraPDF/veraPDF-corpus). It does not install or invoke the Java veraPDF executable; the checked-in result and rule expectation manifests record the expected outcomes for that pinned corpus revision. The gate validates the expected exit status and a matching expected rule without assigning meaning to diagnostic order. The workflow uses a sparse checkout so the gate runs every PDF recursively under the selected profile directories without vendoring the corpus into this repository.

The current selected profiles are PDF/A-1a, PDF/A-1b, PDF/A-2a, PDF/A-2b, PDF/A-2u, PDF/A-3b, and PDF/UA-1. A corpus filename must contain exactly one `-pass-` or `-fail-` marker; its profile is taken from the top-level `PDF_A-*` or `PDF_UA-*` directory, and all nested rule-section directories are included.

The gate requires a `pass` file to make `page` return exit code `0` and a `fail` file to make it return exit code `2`. Exit code `1`, an invalid corpus filename, a missing selected profile directory, or any other operational problem fails the gate with exit code `1`; mismatched validation results fail it with exit code `2`.

In order to ensure no breaking changes when making changes, run the same gate locally with `just verapdf-corpus`; it automatically creates a sparse checkout at `.cache/verapdf-corpus`. Pass an existing checkout as `just verapdf-corpus /path/to/veraPDF-corpus` when preferred. When another validation format is added, update the selected profile list in `crates/page_cli/src/corpus.rs`, the sparse-checkout lists in `justfile` and `.github/workflows/ci.yml`, and this section's documented revision and profile list.
