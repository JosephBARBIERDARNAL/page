# Preliminary Rust PDF/A validator

This project is the first, intentionally narrow milestone toward a PDF/A
validator. It uses `lopdf` for strict PDF parsing and `roxmltree` for bounded
XMP parsing.

It does **not** implement complete PDF/A-1b validation. Passing means only that
the checks listed below found no failure.

## Run

```bash
cargo run -- validate --profile pdfa-1b path/to/file.pdf
cargo run -- validate --profile pdfa-1b --format json path/to/file.pdf
```

The process exits with status `0` when all implemented checks pass, `2` for a
malformed PDF or failed validation check, and `1` for an operational problem
such as unreadable input, a configured safety limit, or report serialization.

## Architecture

```text
bounded file input
    -> strict lopdf parser
    -> normalized PdfDocument model
       (metadata, XMP declaration, output intents, fonts)
    -> preliminary rule evaluator
    -> deterministic text or JSON ValidationReport
```

Operational and parser failures are kept separate from metadata and
conformance failures. Limits are configurable for input bytes, decoded stream
bytes, object count, and reference-chain depth. Operational failures use
`INPUT-IO-001` or `RESOURCE-LIMIT-001` and do not describe PDF conformance.

## Implemented checks

- `PDF-PARSE-001`: the file parses in strict mode.
- `PDFA1B-ENCRYPTION-001`: the document is not encrypted.
- `PDFA1B-CATALOG-001`: the trailer has an indirect Root catalog reference.
- `PDFA1B-XMP-001`: a catalog metadata stream exists and parses as XML.
- `PDFA1B-ID-PART-001`: XMP declares `pdfaid:part` as `1`.
- `PDFA1B-ID-CONFORMANCE-001`: XMP declares `pdfaid:conformance` as the
  case-sensitive value `A` or `B`. The pinned PDF/A-1B veraPDF profile accepts
  level A because it includes the level B requirements.
- `PDFA1B-OUTPUTINTENT-001`: the catalog has at least one output intent.

These identifiers are stable project-local identifiers. The mappings below
make clear which checks correspond to pinned veraPDF rules and which are only
project gates or proxies.

## Differential testing against veraPDF

The `verapdf-diff` binary compares the local subset with an explicitly pinned
veraPDF installation:

```bash
cargo run --bin verapdf-diff -- \
  --verapdf /path/to/verapdf \
  --expected-version 1.28.2 \
  --profile 1b \
  --format text \
  file.pdf another.pdf
```

The reference is veraPDF `1.28.2`, flavour `1b`. The runner first verifies the
executable's version, then invokes each PDF separately with `--loglevel 0`,
`--format json`, and `--flavour 1b`. Disabling veraPDF logging is necessary
because Java warning records can otherwise be inserted into its JSON stdout.
All process arguments are passed directly with `std::process::Command`; no
shell command string is constructed.

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

`agreement`, `both_noncompliant`, and `coverage_gap` exit with status `0`.
Semantic or parser discrepancies exit with status `2`; operational failures
exit with status `1`. Across multiple files, operational status takes
precedence over discrepancy status.

The runner preserves the reference compliant state, processed/rejected state,
failed rule identifiers, failed rule/check counts, task exception, process exit
status, and bounded stdout/stderr excerpts alongside the local
`ValidationReport`. Text and JSON output have deterministic field and failed
rule ordering.

### Pinned rule mapping

The source of truth is
`veraPDF-validation-profiles-rel-1.28/PDF_A/PDFA-1B.xml` installed with
veraPDF 1.28.2.

| Local rule | veraPDF rule | Clause | Strength | Pinned test and semantic note |
|---|---|---|---|---|
| `PDF-PARSE-001` | none | none | none | Operational parser gate, not an ISO conformance rule. |
| `PDFA1B-ENCRYPTION-001` | `ISO_19005_1:6.1.3:2` | ISO 19005-1 §6.1.3 | exact | `isEncrypted != true` |
| `PDFA1B-CATALOG-001` | none | none | none | Local object-model gate; the profile has no standalone catalog-exists rule. |
| `PDFA1B-XMP-001` | `ISO_19005_1:6.7.2:1` | ISO 19005-1 §6.7.2 | partial/proxy | Reference test `containsMetadata == true` also requires stream Type/Subtype, which the local check does not yet verify. XMP serialization is separately covered by reference rule §6.7.9 test 1. |
| `PDFA1B-ID-PART-001` | `ISO_19005_1:6.7.11:2` | ISO 19005-1 §6.7.11 | exact | `part == 1` |
| `PDFA1B-ID-CONFORMANCE-001` | `ISO_19005_1:6.7.11:3` | ISO 19005-1 §6.7.11 | exact | `conformance == "B" \|\| conformance == "A"` |
| `PDFA1B-OUTPUTINTENT-001` | `ISO_19005_1:6.2.2:1` | ISO 19005-1 §6.2.2 | partial/proxy | Merely finding an array entry does not validate `/S`, `DestOutputProfile`, ICC class, colour space, ICC version, or BToA data. |

The same mapping is available as typed Rust data in
`pdf::differential::RULE_MAPPINGS`.

### Opt-in reference suite

Normal tests and the three-OS GitHub workflow do not install or invoke
veraPDF. To run the pinned real-reference cases locally:

```bash
VERAPDF_BIN=/path/to/verapdf \
  cargo test --test verapdf_diff -- --nocapture
```

The case manifest is
`tests/fixtures/verapdf-diff-cases.json`. It covers the structural, malformed,
encrypted, and supplied real-world fixtures with an expected classification
and rationale. The real-world files are not assumed to claim PDF/A.

Small sanitized veraPDF JSON files under `tests/reference-reports/` unit-test
report parsing without an installed reference. Classification, exit-code,
wrong-version, missing-executable, timeout, malformed-report, and spaced-path
behavior are also tested offline.

### Process and fixture safety

- veraPDF gets 30 seconds per invocation by default; a timed-out child is
  killed and reaped.
- Captured reference JSON is limited to 8 MiB. Reader threads continue draining
  both pipes, preventing a full pipe from hanging the child.
- Stored diagnostic excerpts are limited to 16 KiB.
- `serde_json`'s bounded parser depth and the byte limit constrain report
  nesting and allocation.
- Existing local PDF input, decoded-stream, object-count, and reference-depth
  limits remain active.
- Root `.gitattributes` marks `*.pdf binary`, preventing CRLF conversion from
  corrupting PDF xref offsets on Windows.
- `tests/fixture_integrity.rs` pins SHA-256 hashes for every PDF fixture so
  line-ending or other byte changes fail clearly on every platform.

## Known limitations

- This is not a complete PDF/A conformance checker.
- Output intents are detected but their ICC profiles are not validated.
- Fonts are summarized, but font programs and embedding requirements are not
  validated.
- XMP extraction covers the PDF/A identification namespace but not complete
  RDF/XMP schema validation or Info/XMP consistency.
- PDFDocEncoding is not fully decoded; non-Unicode Info strings fall back to
  UTF-8 loss replacement.
- `lopdf` provides the normalized object graph, so syntax provenance such as
  duplicate dictionary keys and original token spellings is not retained.
- Reference-depth limits protect this crate's normalization traversals, not
  every internal traversal performed by the parser.
- Password-protected input is detected from the loaded trailer and fails the
  encryption check without traversing inaccessible objects. The regression
  fixture covers AES-256 encryption generated by qpdf; other encryption
  revisions remain subject to `lopdf`'s parser support.
- The included `structural.pdf` is a parser fixture, not a PDF/A-conforming
  document.
- Differential agreement covers only the implemented checks and selected
  pinned reference. It does not establish full PDF/A-1b conformance.
