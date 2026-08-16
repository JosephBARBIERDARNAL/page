---
title: PDF/A-2 and PDF/A-3 pinned rule mapping
---

<!-- The machine-readable source of truth is `crates/page_validation/tests/fixtures/pdfa-2-3-coverage.json`. -->

PDF/A-2 and PDF/A-3 share the core PDF/A rule families but differ from PDF/A-1 in the capabilities that the profile evaluator selects at runtime.

| Profile family | Pinned veraPDF profiles | Local profile selection |
| --- | --- | --- |
| PDF/A-2a/b/u | `2a`, `2b`, `2u` from veraPDF 1.30.2 | Part 2, with A requiring tagged structure and A/U requiring Unicode mappings. |
| PDF/A-3a/b/u | `3a`, `3b`, `3u` from veraPDF 1.30.2 | Part 3, with the same level requirements as PDF/A-2 and associated-file capability. |

The pinned inventory records each profile's checked-in XML source, SHA-256, and exact veraPDF predicate count. The local rule families are shared with PDF/A-1 where semantics are unchanged, then renamed to the selected `PDFA2*`, `PDFA2U`, `PDFA3*`, or `PDFA3U` namespace in the report.

The `predicate_ids` arrays in [`pdfa-2-3-coverage.json`](../../crates/page_validation/tests/fixtures/pdfa-2-3-coverage.json) enumerate every rule ID from each checked-in profile; the inventory test parses the XML, verifies the hash and count, and rejects missing or duplicate IDs.

| Profile | Pinned profile source |
| --- | --- |
| PDF/A-2a | [`PDFA-2A-1.30.2.xml`](../../crates/page_validation/tests/fixtures/PDFA-2A-1.30.2.xml) |
| PDF/A-2b | [`PDFA-2B-1.30.2.xml`](../../crates/page_validation/tests/fixtures/PDFA-2B-1.30.2.xml) |
| PDF/A-2u | [`PDFA-2U-1.30.2.xml`](../../crates/page_validation/tests/fixtures/PDFA-2U-1.30.2.xml) |
| PDF/A-3a | [`PDFA-3A-1.30.2.xml`](../../crates/page_validation/tests/fixtures/PDFA-3A-1.30.2.xml) |
| PDF/A-3b | [`PDFA-3B-1.30.2.xml`](../../crates/page_validation/tests/fixtures/PDFA-3B-1.30.2.xml) |
| PDF/A-3u | [`PDFA-3U-1.30.2.xml`](../../crates/page_validation/tests/fixtures/PDFA-3U-1.30.2.xml) |

PDF/A-2/3-specific capability decisions are explicit in the shared evaluator: xref streams, optional content, transparency, JPEG2000-compatible image rules, 16-bit image components, PDF/A-2/3 DeviceN limits and Colorants, PDF/A-2/3 blend modes, OpenType font programs, halftone restrictions, signature `/ByteRange` coverage, and PDF/A-3 embedded files. Differential classifications continue to distinguish agreement and both-noncompliant results from coverage gaps.
