# PDF/A-1B completion gate

`tag` must not describe a local pass as PDF/A-1B compliant while the checked-in
coverage inventory is marked `developing`. The source of truth is veraPDF
1.28.2, flavour `1b`, and the byte-pinned
`PDFA-1B-1.28.xml` validation profile.

The machine-readable inventory is
`crates/tag_validation/tests/fixtures/pdfa-1b-coverage.json`. It contains all
129 unique profile predicates, their local rule mappings, implementation
strength, pass/fail/inapplicable evidence, variant matrix, parser matrix,
integration corpus, fixture hashes, and bounded mutation recipes.

Run the offline inventory check with:

```sh
just coverage
```

Run every centralized atomic and corpus comparison with the pinned executable:

```sh
VERAPDF_BIN=/path/to/verapdf just verapdf
```

Regenerate the predicate portion of the inventory after changing the pinned
profile, rule mapping, or atomic manifest:

```sh
cargo test -p tag_validation --test coverage_inventory \
  regenerate_coverage_inventory -- --ignored
just coverage
```

The generated file is reviewed and checked in. CI verifies it against the
pinned XML, the differential case manifest, the local mapping table, and the
fixture bytes.

## Low-level syntax milestone

The inventory's `low_level_syntax` matrix covers all 30 clause 6.1 predicates.
Its 26 file-syntax and selected-COS-object predicates are exact against
veraPDF 1.28.2. The remaining four entries belong to bounded content execution
or embedded-program inspection and retain the strength of their owning
coverage families; they are not partial because raw COS provenance is missing.

The internal source layer retains revision-selected object spans, duplicate
dictionary entries, original scalar spellings, decoded name and string values,
classic-xref and trailer provenance, and independent stream boundaries.
Recoverable invalid hexadecimal and xref syntax is reported through the pinned
PDF/A rule rather than collapsed into `PDF-PARSE-001`. Fatal malformed input and
configured resource limits remain parser and operational outcomes,
respectively.

The centralized `syntax` differential family pins direct and indirect values,
last-duplicate-key behavior, incremental replacement, linearized trailers,
decoded boundary values, stream length and external-data keys, and malformed
recovery. Regenerate only the matrix after changing its routing policy with:

```sh
cargo test -p tag_validation --test coverage_inventory \
  regenerate_low_level_syntax_matrix -- --ignored
```

## Declaring the profile complete

Change `completion_gate.status` to `complete` only after all of these conditions
are true:

1. All 129 predicates have centralized veraPDF rule-ID delta evidence for an
   applicable pass and applicable fail.
2. Every representable inapplicable state and every object-model distinction
   that changes veraPDF behavior has a fixture.
3. Direct/indirect, null/wrong-type, inherited, nested, cyclic, malformed, and
   parser-recovery matrices are complete.
4. Every local mapping has `exact` strength.
5. The parser and cross-family corpora have no unexpected semantic or parser
   discrepancy.
6. No checked-in case expects `coverage_gap`.
7. `completion_gate.coverage_gap_is_success` is `false`.
8. `just check` and the full opt-in veraPDF suite pass on every supported CI
   platform.

The release-only command enforces the declaration and then runs the live suite:

```sh
VERAPDF_BIN=/path/to/verapdf just pdfa1b-release-gate
```

It intentionally fails while the inventory remains `developing`. A manual
GitHub Actions run can enable `pdfa1b_release_gate` to run the same release
check.

After completion, invoke `verapdf-diff --require-complete` in release workflows.
That policy changes `coverage_gap` from an acceptable development
classification to exit status `2`. Operational failures remain status `1`;
unexpected semantic and parser discrepancies remain status `2`.
