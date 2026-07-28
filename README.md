# mai

mai's goal is to provide a fully API-compatible veraPDF alternative written in
Rust. veraPDF is the source of truth for the expected output of a given
validation.

The current implementation is an intentionally narrow milestone toward that
goal. It uses `lopdf` for strict PDF parsing and `roxmltree` for bounded XMP
parsing.

It does **not** implement complete PDF/A-1b validation. Passing means only that
the checks listed below found no failure.

The repository is a Cargo workspace with two packages:

- `mai-validation` contains the reusable parser, normalized model, validation
  rules, reports, safety limits, and veraPDF differential engine.
- `mai-cli` contains client-side argument parsing, output selection, process
  exit behavior, and the `mai` and `verapdf-diff` executables. It depends on
  `mai-validation` through a workspace path dependency.

Validation internals and the CLI are kept separate: `mai-validation` must not
depend on `mai-cli` or CLI-only dependencies.

## Run

```bash
cargo run -p mai-cli --bin mai -- validate --profile pdfa-1b path/to/file.pdf
cargo run -p mai-cli --bin mai -- validate --profile pdfa-1b --format json path/to/file.pdf
```

The process exits with status `0` when all implemented checks pass, `2` for a
malformed PDF or failed validation check, and `1` for an operational problem
such as unreadable input, a configured safety limit, or report serialization.

## Architecture

```text
mai-cli
    -> argument and output handling
    -> mai-validation
       -> bounded file input
       -> strict lopdf parser
       -> normalized PdfDocument model
          (metadata, XMP declaration, output intents, fonts)
       -> preliminary rule evaluator
       -> deterministic ValidationReport
```

Operational and parser failures are kept separate from metadata and
conformance failures. Limits are configurable for input bytes, decoded stream
bytes, object count, and reference-chain depth. Operational failures use
`INPUT-IO-001` or `RESOURCE-LIMIT-001` and do not describe PDF conformance.
Library tests and fixtures live under `crates/mai-validation/tests`; CLI
contract tests live under `crates/mai-cli/tests`. Each package declares only
the dependencies it uses.

## Implemented checks

- `PDF-PARSE-001`: the file parses in strict mode.
- `PDFA1B-ENCRYPTION-001`: the document is not encrypted.
- `PDFA1B-CATALOG-001`: the trailer has an indirect Root catalog reference.
- `PDFA1B-METADATA-STRUCTURE-001`: catalog `/Metadata` resolves to a stream
  with `/Type /Metadata` and `/Subtype /XML`.
- `PDFA1B-METADATA-FILTER-001`: the catalog metadata stream has no `/Filter`.
- `PDFA1B-XMP-001`: the metadata bytes parse as bounded, DTD-disabled XML.
- `PDFA1B-ID-SCHEMA-001`: XMP contains a property in the PDF/A identification
  namespace.
- `PDFA1B-ID-PART-001`: XMP declares `pdfaid:part` as `1`.
- `PDFA1B-ID-CONFORMANCE-001`: XMP declares `pdfaid:conformance` as the
  case-sensitive value `A` or `B`. The pinned PDF/A-1B veraPDF profile accepts
  level A because it includes the level B requirements.
- `PDFA1B-INFO-{CREATIONDATE,TITLE,AUTHOR,SUBJECT,KEYWORDS,CREATOR,PRODUCER,MODDATE}-001`:
  the corresponding Info value, when present, agrees with its XMP analogue.
  Title and Subject use `rdf:Alt` `x-default`; Author uses an `rdf:Seq` with
  exactly one item. Common full dates are compared as instants, including
  equivalent timezone offsets.
- `PDFA1B-OUTPUTINTENT-001`: each safely decoded `DestOutputProfile` stream
  linked from an output-intent dictionary has ICC class `prtr` or `mntr`,
  colour space `RGB `, `CMYK`, or `GRAY`, and a major/minor version below 3.0.
  This stable identifier previously represented a coarse presence proxy; it
  now represents the pinned §6.2.2 test-1 predicate.
- `PDFA1B-OUTPUTINTENT-IDENTITY-001`: every indirect
  `DestOutputProfile` value in the array identifies the same indirect object.
  Missing and direct values are ignored, matching the pinned predicate.

These identifiers are stable project-local identifiers. The mappings below
make clear which checks correspond to pinned veraPDF rules and which are only
project gates or proxies.

## Differential testing against veraPDF

The `verapdf-diff` binary compares the local subset with an explicitly pinned
veraPDF installation:

```bash
cargo run -p mai-cli --bin verapdf-diff -- \
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
| `PDFA1B-ENCRYPTION-001` | `ISO 19005-1:2005:6.1.3:2` | ISO 19005-1 §6.1.3 | exact | `isEncrypted != true` |
| `PDFA1B-CATALOG-001` | none | none | none | Local object-model gate; the profile has no standalone catalog-exists rule. |
| `PDFA1B-METADATA-STRUCTURE-001` | `ISO 19005-1:2005:6.7.2:1` | §6.7.2 | exact | `containsMetadata == true` |
| `PDFA1B-METADATA-FILTER-001` | `ISO 19005-1:2005:6.7.2:2` | §6.7.2 | exact | `isCatalogMetadata == false \|\| Filter == null` |
| `PDFA1B-XMP-001` | `ISO 19005-1:2005:6.7.9:1` | §6.7.9 | partial/proxy | XML well-formedness is necessary, but does not implement the complete XMP 2004 serialization and extension-schema model. |
| `PDFA1B-ID-SCHEMA-001` | `ISO 19005-1:2005:6.7.11:1` | §6.7.11 | partial/proxy | Common packages agree; veraPDF recovery differs for invalid duplicate packages. |
| `PDFA1B-ID-PART-001` | `ISO 19005-1:2005:6.7.11:2` | §6.7.11 | partial/proxy | `part == 1`; common single-property packages agree. |
| `PDFA1B-ID-CONFORMANCE-001` | `ISO 19005-1:2005:6.7.11:3` | §6.7.11 | partial/proxy | `conformance == "B" \|\| conformance == "A"`; common single-property packages agree. |
| `PDFA1B-INFO-CREATIONDATE-001` | `ISO 19005-1:2005:6.7.3:1` | §6.7.3 | partial/proxy | Common full dates are compared as instants; reduced-precision XMP forms are not implemented. |
| `PDFA1B-INFO-TITLE-001` | `ISO 19005-1:2005:6.7.3:2` | §6.7.3 | partial/proxy | ASCII `dc:title` `rdf:Alt` `x-default` cases agree. |
| `PDFA1B-INFO-AUTHOR-001` | `ISO 19005-1:2005:6.7.3:3` | §6.7.3 | partial/proxy | ASCII `dc:creator` `rdf:Seq` equality and one-item multiplicity agree. |
| `PDFA1B-INFO-SUBJECT-001` | `ISO 19005-1:2005:6.7.3:4` | §6.7.3 | partial/proxy | ASCII `dc:description` `rdf:Alt` `x-default` cases agree. |
| `PDFA1B-INFO-KEYWORDS-001` | `ISO 19005-1:2005:6.7.3:5` | §6.7.3 | partial/proxy | ASCII `pdf:Keywords` cases agree. |
| `PDFA1B-INFO-CREATOR-001` | `ISO 19005-1:2005:6.7.3:6` | §6.7.3 | partial/proxy | ASCII `xmp:CreatorTool` cases agree. |
| `PDFA1B-INFO-PRODUCER-001` | `ISO 19005-1:2005:6.7.3:7` | §6.7.3 | partial/proxy | ASCII `pdf:Producer` cases agree. |
| `PDFA1B-INFO-MODDATE-001` | `ISO 19005-1:2005:6.7.3:8` | §6.7.3 | partial/proxy | Common full dates are compared as instants; reduced-precision XMP forms are not implemented. |
| `PDFA1B-OUTPUTINTENT-001` | `ISO 19005-1:2005:6.2.2:1` | §6.2.2 | exact | For safely decoded linked streams: `(deviceClass == "prtr" \|\| deviceClass == "mntr") && (colorSpace == "RGB " \|\| colorSpace == "CMYK" \|\| colorSpace == "GRAY") && version < 3.0`. |
| `PDFA1B-OUTPUTINTENT-IDENTITY-001` | `ISO 19005-1:2005:6.2.2:2` | §6.2.2 | exact | `sameOutputProfileIndirect == true`; missing/direct values are ignored and indirect non-stream targets participate by identity. |

The same mapping is available as typed Rust data in
`mai_validation::differential::RULE_MAPPINGS`.

### Opt-in reference suite

Normal tests and the three-OS GitHub workflow do not install or invoke
veraPDF. To run the pinned real-reference cases locally:

```bash
VERAPDF_BIN=verapdf \
  cargo test -p mai-validation --test verapdf_diff -- --nocapture
```

The case manifest is
`crates/mai-validation/tests/fixtures/verapdf-diff-cases.json`. In addition
to the structural, malformed, encrypted, and supplied real-world fixtures, it
defines deterministic atomic metadata and output-intent cases. The opt-in suite
generates those PDFs at runtime and compares both local and veraPDF
failed-rule-ID deltas against a baseline for each group. This prevents
unrelated known PDF/A failures in the small generated documents from hiding
targeted regressions.

The live comparison deliberately records two pinned-model distinctions:

- conflicting duplicate identification descriptions fail veraPDF XMP
  serialization/schema-presence rules rather than its part/conformance rules;
- an invalid XMP date fails veraPDF's property-type rule §6.7.9 test 3 rather
  than the Info/date-equivalence rule.

The output-intent cases also pin several veraPDF 1.28.2 model behaviors that
are narrower than the prose description:

- absent, non-array, empty, and dictionary-free `/OutputIntents` values make
  both §6.2.2 rules inapplicable;
- test 1 is instantiated for any destination profile stream and does not gate
  on `/S`; its predicate reads only ICC version, device class, and data colour
  space fields;
- test 2 compares indirect object identity rather than profile bytes, ignores
  missing and direct values, and includes indirect values whose targets are
  not streams.

Small sanitized veraPDF JSON files under
`crates/mai-validation/tests/reference-reports/` unit-test report parsing
without an installed reference. Classification, exit-code, wrong-version,
missing-executable, timeout, malformed-report, and spaced-path behavior are
also tested offline.

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
- Output-intent validation intentionally covers only the two predicates in the
  pinned §6.2.2 profile. It does not establish general ICC validity: declared
  profile size, `acsp` signature, PCS, tag table, rendering transforms, and
  BToA data are not inspected by these predicates. A 20-byte prefix can
  therefore pass test 1 when its three exposed fields pass.
- Missing `/OutputIntents`, missing `/S`, and missing `DestOutputProfile` are
  not standalone failures in this milestone because veraPDF 1.28.2 creates no
  failing §6.2.2 model predicate for those cases.
- Fonts are summarized, but font programs and embedding requirements are not
  validated.
- XMP extraction implements a bounded typed subset for identification,
  standard Info analogues, `rdf:Alt`, and `rdf:Seq`; it is not a complete XMP
  2004 data model or extension-schema validator.
- PDF/A-1 §6.7.11 tests 4–6 (required lexical prefixes for `part`,
  `conformance`, and `amd`) are not implemented. Namespace-aware XML parsing
  intentionally does not retain the lexical prefix used by each property.
- PDF/A-1 §6.7.9 tests 2–3 (schema definition and complete XMP value typing)
  are not implemented.
- `Trapped` has no Info/XMP consistency predicate in the pinned PDF/A-1B
  profile's §6.7.3 rules, so this milestone does not invent one.
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

## Development commands

The repository includes a `justfile` so the common commands have one stable
interface. Run `just` to list them. The main workflows are:

```bash
just check
just build
just test-verapdf
just validate crates/mai-validation/tests/fixtures/structural.pdf
just validate crates/mai-validation/tests/fixtures/structural.pdf json
just diff crates/mai-validation/tests/fixtures/structural.pdf
```

`just check` runs formatting verification, strict Clippy, and the complete
offline test suite. `just test-verapdf` resolves `verapdf` from `PATH` by
default; set `VERAPDF_BIN` to override it.

## Implementation status

The validator currently exposes 19 checks. Five are exact mappings to the
pinned veraPDF 1.28.2 PDF/A-1B profile, twelve are deliberately bounded
partial/proxy implementations, and two are local parser/model gates.

### Implemented exactly

- `PDFA1B-ENCRYPTION-001` — rejects encrypted documents.
- `PDFA1B-METADATA-STRUCTURE-001` — requires catalog metadata to resolve to a
  `/Type /Metadata`, `/Subtype /XML` stream.
- `PDFA1B-METADATA-FILTER-001` — rejects a filter on the catalog metadata
  stream.
- `PDFA1B-OUTPUTINTENT-001` — implements pinned §6.2.2 test 1 for ICC device
  class, data colour space, and version.
- `PDFA1B-OUTPUTINTENT-IDENTITY-001` — implements pinned §6.2.2 test 2 for
  indirect destination-profile object identity.

### Implemented as bounded partial/proxy checks

- `PDFA1B-XMP-001` — bounded, DTD-disabled XML parsing, not the complete XMP
  2004 serialization model.
- `PDFA1B-ID-SCHEMA-001`, `PDFA1B-ID-PART-001`, and
  `PDFA1B-ID-CONFORMANCE-001` — the common PDF/A identification schema,
  part, and conformance cases.
- `PDFA1B-INFO-CREATIONDATE-001`, `PDFA1B-INFO-TITLE-001`,
  `PDFA1B-INFO-AUTHOR-001`, `PDFA1B-INFO-SUBJECT-001`,
  `PDFA1B-INFO-KEYWORDS-001`, `PDFA1B-INFO-CREATOR-001`,
  `PDFA1B-INFO-PRODUCER-001`, and `PDFA1B-INFO-MODDATE-001` — common
  Info-to-XMP consistency cases.

### Implemented as local gates

- `PDF-PARSE-001` — the input must parse within configured resource limits.
- `PDFA1B-CATALOG-001` — the trailer root must resolve to a catalog
  dictionary.

### Still required for full PDF/A-1B validation

- Complete PDF file and object syntax validation, including all header,
  trailer, cross-reference, stream, dictionary, and numeric constraints.
- The remaining colour requirements: full ICC profile validation, calibrated
  and device colour-space rules, rendering intents, and every output-intent
  rule outside the two implemented §6.2.2 predicates.
- Font conformance: mandatory embedding, font descriptors and programs,
  encodings, glyph coverage, metrics, and Unicode mappings.
- Graphics and content-stream rules, including operators, images, transparency,
  patterns, shadings, and extended graphics state.
- Restrictions for annotations, actions, forms, optional content, multimedia,
  embedded files, JavaScript, and other interactive features.
- The complete XMP 2004 data model, extension-schema validation, value typing,
  lexical-prefix rules, and complete PDFDocEncoding support.
- Remaining document-structure requirements, including name trees, logical
  structure when present, page-tree details, and other catalog-level
  constraints.
- Exhaustive veraPDF rule coverage and differential fixtures for every
  implemented predicate and parser boundary.

PDF/A-1A-specific requirements and the PDF/A-2, PDF/A-3, and PDF/A-4 families
have not been implemented. Passing all current checks therefore means only
that this documented subset passed; it is not proof of PDF/A conformance.
