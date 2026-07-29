# mai

mai's goal is to provide a fully API-compatible veraPDF alternative written in Rust.

The current implementation is an intentionally narrow milestone toward that goal. It uses `lopdf` for strict PDF parsing and `roxmltree` for bounded XMP parsing.

It does **not** implement complete PDF/A-1b validation. Passing means only that the checks listed below found no failure.

The repository is a Cargo workspace with two packages:

- `mai-validation` contains the reusable parser, normalized model, validation
  rules, reports, safety limits, and veraPDF differential engine.
- `mai-cli` contains client-side argument parsing, output selection, process
  exit behavior, and the `mai` and `verapdf-diff` executables. It depends on
  `mai-validation` through a workspace path dependency.

Validation internals and the CLI are kept separate: `mai-validation` must not
depend on `mai-cli` or CLI-only dependencies.



## Roadmap

- [ ] PDF/A-1b (1.4)
    - https://github.com/veraPDF/veraPDF-validation-profiles/wiki/PDFA-Part-1-rules/
- [ ] PDF/A-1a (1.4)
- [ ] PDF/A-2 (1.7)
- [ ] PDF/A-3 (1.7)
- [ ] PDF/A-4 (2.0)

