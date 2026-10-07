# Repository Guidelines

## Overview

page is a new, modern, fast and lightweight PDF accessibility and compliance checker. It provides an alternative to veraPDF written in Rust.

## Miscellaneous rules

- When veraPDF produces a very unexpected result, determine whether it is an upstream bug. To establish that, create a minimal reproducible example.
- Keep each Markdown paragraph and bullet item on a single line; use line breaks only between paragraphs, headings, and list items.
- Every Rust source file under `crates/page_validation/src/`, including newly added files, must start with a `//!` overview explaining its purpose, overall responsibility, and how it works, in at most two paragraphs. Keep the overview accurate when changing the file's responsibilities.
- When implementing a rule or feature, consider its performance impact and address low-cost improvements.
- Reuse logic across profiles when it fits the rule semantics.
- Treat every git change you did not explicitly make as belonging to someone else working on the project simultaneously; preserve it exactly and do not revert, restore, overwrite, format, or otherwise “clean up” that file.
- If a test, formatter, generator, or other command unexpectedly changes a tracked file, do not assume the command owns the change and do not restore it from `HEAD`; preserve the change and report it or ask before touching it.
- Do not add `#[allow(...)]` attributes or weaken lint settings without first explaining the specific need and getting approval.
- Prefer direct implementations over unnecessary abstractions.
- For a fix or feature, add an entry under the Dev section of `docs/changelog.md` in the existing format. Most entries reference a GitHub issue; ask the user for the issue number when one is not provided.
- After code changes, run the relevant format, lint, type-check, and unit-test commands from `justfile`. `just check` runs checks for Rust and both bindings, but does not reproduce every CI workflow.

## Project Structure & Module Organization

This Rust 2024 project is a virtual Cargo workspace with 4 packages:

- page_validation: core crate with the most code and the validation engine
- page_cli: CLI for the engine
- page_python: Python bindings with Pyo3
- page_wasm: Wasm bindings

Keep reusable PDF parsing, normalization, validation rules, reports, and safety limits in `crates/page_validation`. Keep CLI argument parsing, presentation, exit behavior, and executable entry points in `crates/page_cli`. Keep internal validation logic separate from the CLI. The CLI may depend on the validation crate; the validation crate must never depend on the CLI crate or on client-only dependencies such as Clap.

Bindings should adapt the core crate (`page_validation`) to each language while keeping validation behavior and report semantics consistent. Keep client-specific API and presentation details in the binding crate.

Validation unit and integration tests live with `page_validation`; keep its shared helpers in `crates/page_validation/tests/common/` and PDF inputs in `crates/page_validation/tests/fixtures/`. CLI contract tests live in `crates/page_cli/tests/`. Python binding tests live in `crates/page_python/tests/`, and Wasm tests live in `crates/page_wasm/tests/`. Build artifacts under `target/` are not source files.

## Performance and benchmark

Performance is measured against veraPDF in `bench/benchmark.py`, using five documents and three PDF profiles. The benchmark takes several minutes; run it when a change is likely to affect performance, rather than for routine edits.

## Security

By default, page enforces resource limits called safety limits. See `docs/guide/safety-limits.md`.

## Versioning

page uses semantic versioning. All crates must share the exact same version; check `[workspace.package].version` in `Cargo.toml` rather than relying on a version copied into this file.

## Testing Guidelines

Place focused unit tests beside their owning validation modules in `#[cfg(test)]` blocks and validation integration behavior in `crates/page_validation/tests/*.rs`. Test CLI argument parsing, output formats, and exit status in `crates/page_cli/tests/*.rs`; test Python behavior in `crates/page_python/tests/` and Wasm behavior in `crates/page_wasm/tests/`. Name tests for observable behavior, for example `rejects_encrypted_pdf`, and add regression coverage to the package that owns the behavior. There is no numeric coverage target. PDF fixtures are binary and hash-pinned by `crates/page_validation/tests/fixture_integrity.rs`; update fixtures and their expected hashes intentionally.

Use `just check` for formatting, lint, type, and test checks across Rust and both bindings. For faster binding-specific iteration, use `just py-check` or `just wasm-check`; for changes to supported corpus profiles or rule behavior, run `just verapdf-corpus` when practical. CI also checks dependency licenses and runs platform-specific jobs, so `just check` is not a complete local reproduction of CI. Run `just typst` when regenerating Typst PDF fixtures; preserve unrelated fixture changes and update pinned hashes intentionally.

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

## Implementing a new rule

When asked to implement a new rule for a PDF profile, verify whether it has already been implemented for another profile in order to reuse it. Add a unit test to verify its behavior, and check whether the veraPDF corpus conformance gate has a test for it.

## veraPDF corpus conformance gate

The required pull-request CI check runs `page corpus` against the pinned `staging` revision `49de56cd987929932c9e4fbbbe67d052bf44ef83` of the external [veraPDF corpus](https://github.com/veraPDF/veraPDF-corpus). It does not install or invoke the Java veraPDF executable; the checked-in result and rule expectation manifests record the expected outcomes for that pinned corpus revision. The gate validates the expected exit status and a matching expected rule without assigning meaning to diagnostic order. The workflow uses a sparse checkout so the gate runs every PDF recursively under the selected profile directories without vendoring the corpus into this repository.

The current selected profiles are PDF/A-1a, PDF/A-1b, PDF/A-2a, PDF/A-2b, PDF/A-2u, PDF/A-3b, and PDF/UA-1. A corpus filename must contain exactly one `-pass-` or `-fail-` marker; its profile is taken from the top-level `PDF_A-*` or `PDF_UA-*` directory, and all nested rule-section directories are included.

The gate requires a `pass` file to make `page` return exit code `0` and a `fail` file to make it return exit code `2`. Exit code `1`, an invalid corpus filename, a missing selected profile directory, or any other operational problem fails the gate with exit code `1`; mismatched validation results fail it with exit code `2`.

To check for corpus regressions, run `just verapdf-corpus`; it automatically creates a sparse checkout at `.cache/verapdf-corpus`. Pass an existing checkout as `just verapdf-corpus /path/to/veraPDF-corpus` when preferred. When adding a supported corpus profile, update `crates/page_cli/src/corpus_profiles.txt`, the profile list used by this section, and the matching sparse-checkout inputs in `justfile`. The workflow in `.github/workflows/corpus.yml` invokes that just target, so it has no separate profile list. Update the documented corpus revision when the pinned revision changes.
