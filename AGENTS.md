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

The source of truth is `veraPDF-validation-profiles-rel-1.28/PDF_A/PDFA-1B.xml` installed with veraPDF 1.28.2.

| Local rule | veraPDF rule | Clause | Strength | Pinned test and semantic note |
|---|---|---|---|---|
| `PDF-PARSE-001` | none | none | none | Operational parser gate, not an ISO conformance rule. |
| `PDFA1B-ENCRYPTION-001` | `ISO 19005-1:2005:6.1.3:2` | ISO 19005-1 §6.1.3 | exact | `isEncrypted != true` |
| `PDFA1B-CATALOG-001` | none | none | none | Local object-model gate; the profile has no standalone catalog-exists rule. |
| `PDFA1B-HEADER-001` | `ISO 19005-1:2005:6.1.2:1` | §6.1.2 | exact | The `%` marker of the file header occurs at byte offset 0 and is immediately followed by a `PDF-`major`.`minor version declaration. |
| `PDFA1B-HEADER-BINARY-COMMENT-001` | `ISO 19005-1:2005:6.1.2:2` | §6.1.2 | exact | The header line is immediately followed by a comment whose first four bytes each have a decimal value above 127. |
| `PDFA1B-TRAILER-ID-001` | `ISO 19005-1:2005:6.1.3:1` | §6.1.3 | exact | The applicable revision trailer is the first-page trailer for a validated linearized file and otherwise the last trailer. Any direct `ID` array is non-null in veraPDF's accessor, including empty and one-item arrays; string elements are concatenated and wrong-type elements are ignored. |
| `PDFA1B-POST-EOF-DATA-001` | `ISO 19005-1:2005:6.1.3:3` | §6.1.3 | exact | No bytes follow the last `%%EOF` marker except a single optional end-of-line marker. |
| `PDFA1B-LINEARIZED-TRAILER-ID-001` | `ISO 19005-1:2005:6.1.3:4` | §6.1.3 | exact | In a linearized file, if `ID` is present in both the first-page and last trailer, the two values are identical. |
| `PDFA1B-HEX-STRING-LENGTH-001` | `ISO 19005-1:2005:6.1.6:1` | §6.1.6 | exact | Every modeled hexadecimal string retains its original non-whitespace digit count, including veraPDF-recoverable malformed strings, and that count is even. |
| `PDFA1B-HEX-STRING-CHARACTERS-001` | `ISO 19005-1:2005:6.1.6:2` | §6.1.6 | exact | Every modeled hexadecimal string retains whether all original non-whitespace characters were `0`-`9`, `A`-`F`, or `a`-`f`; a recoverable invalid character is a conformance failure rather than `PDF-PARSE-001`. |
| `PDFA1B-STREAM-LENGTH-001` | `ISO 19005-1:2005:6.1.7:1` | §6.1.7 | exact | The effective direct or indirect `/Length`, after last-duplicate-key selection, matches the independent raw byte extent from the first data byte through the byte before the EOL preceding `endstream`. |
| `PDFA1B-STREAM-EOL-001` | `ISO 19005-1:2005:6.1.7:2` | §6.1.7 | exact | The `stream` keyword is followed by CRLF or a single LF, and `endstream` is preceded by an end-of-line marker. |
| `PDFA1B-INDIRECT-OBJECT-SYNTAX-001` | `ISO 19005-1:2005:6.1.8:1` | §6.1.8 | exact | An indirect object's number and generation are separated from each other and from the `obj`/`endobj` keywords by single whitespace, bounded by end-of-line markers. |
| `PDFA1B-XREF-SUBSECTION-SPACING-001` | `ISO 19005-1:2005:6.1.4:1` | §6.1.4 | exact | Each classic-xref subsection in the `/Prev` and `/XRefStm` revision graph separates its starting object number and range with a single SPACE; recoverable alternate separators remain modeled failures. |
| `PDFA1B-XREF-EOL-001` | `ISO 19005-1:2005:6.1.4:2` | §6.1.4 | exact | The `xref` keyword and each subsection header in every selected revision are separated by a single end-of-line marker; recoverable extra markers remain modeled failures. |
| `PDFA1B-XREF-STREAM-001` | `ISO 19005-1:2005:6.1.4:3` | §6.1.4 | exact | The document contains no cross-reference stream. |
| `PDFA1B-METADATA-STRUCTURE-001` | `ISO 19005-1:2005:6.7.2:1` | §6.7.2 | exact | `containsMetadata == true` |
| `PDFA1B-METADATA-FILTER-001` | `ISO 19005-1:2005:6.7.2:2` | §6.7.2 | exact | `isCatalogMetadata == false \|\| Filter == null` |
| `PDFA1B-XMP-001` | `ISO 19005-1:2005:6.7.9:1` | §6.7.9 | partial/proxy | XML well-formedness is necessary, but does not implement the complete XMP 2004 serialization and extension-schema model. |
| `PDFA1B-XMP-PREDEFINED-PROPERTY-001` | `ISO 19005-1:2005:6.7.9:2` | §6.7.9 | partial/proxy | Every element or attribute in a predefined XMP2004 namespace is a property that namespace defines, over the bounded `xmp2004_properties.txt` table. |
| `PDFA1B-XMP-PREDEFINED-VALUE-TYPE-001` | `ISO 19005-1:2005:6.7.9:3` | §6.7.9 | partial/proxy | Every predefined-namespace property's value matches its predefined value-type shape; pinned live by the opt-in `invalid_gps_coordinate_matches_pinned_verapdf_when_opted_in` case, which confirms veraPDF reports this same test for an invalid GPS coordinate. |
| `PDFA1B-XMP-EXTENSION-PROPERTY-DEFINITION-001` | `ISO 19005-1:2005:6.7.9:2` | §6.7.9 | partial/proxy | Every property used in the XMP body outside the predefined namespaces is declared by the document's own extension schemas. |
| `PDFA1B-XMP-EXTENSION-PROPERTY-VALUE-SHAPE-001` | `ISO 19005-1:2005:6.7.9:3` | §6.7.9 | partial/proxy | Every used extension property's value matches its extension-schema-declared value type; pinned live by the opt-in `invalid_extension_xpath_matches_pinned_verapdf_when_opted_in` case, which confirms veraPDF reports this same test for an invalid extension property value. |
| `PDFA1B-XMP-PACKET-BYTES-001` | `ISO 19005-1:2005:6.7.5:1` | §6.7.5 | exact | On the last `xpacket` processing instruction encountered before the main RDF node: `bytes == null`, using veraPDF's case-sensitive, unanchored quoted-assignment matcher. |
| `PDFA1B-XMP-PACKET-ENCODING-001` | `ISO 19005-1:2005:6.7.5:2` | §6.7.5 | exact | On the same selected packet header: `encoding == null`, using the corresponding case-sensitive, unanchored quoted-assignment matcher. |
| `PDFA1B-XMP-EXTENSION-FIELDS-001` | `ISO 19005-1:2005:6.7.8:1` | §6.7.8 | partial/proxy | For bounded common extension objects: `containsUndefinedFields == false`, using the pinned namespace and allowed-child sets for schema definitions, properties, value types, and fields. |
| `PDFA1B-XMP-EXTENSION-CONTAINER-001` | `ISO 19005-1:2005:6.7.8:2` | §6.7.8 | partial/proxy | For modeled `pdfaExtension:schemas`: `isValidBag == true && prefix == "pdfaExtension"`. |
| `PDFA1B-XMP-EXTENSION-SCHEMA-NAME-001` | `ISO 19005-1:2005:6.7.8:3` | §6.7.8 | partial/proxy | For modeled schema definitions: `isSchemaValidText == true && schemaPrefix == "pdfaSchema"`. |
| `PDFA1B-XMP-EXTENSION-SCHEMA-NAMESPACE-001` | `ISO 19005-1:2005:6.7.8:4` | §6.7.8 | partial/proxy | `isNamespaceURIValidURI == true && namespaceURIPrefix == "pdfaSchema"`; the pinned URI validator requires a simple XMP node. |
| `PDFA1B-XMP-EXTENSION-SCHEMA-PREFIX-001` | `ISO 19005-1:2005:6.7.8:5` | §6.7.8 | partial/proxy | `isPrefixValidText == true && prefixPrefix == "pdfaSchema"`. |
| `PDFA1B-XMP-EXTENSION-SCHEMA-PROPERTIES-001` | `ISO 19005-1:2005:6.7.8:6` | §6.7.8 | partial/proxy | `isPropertyValidSeq == true && (propertyPrefix == null \|\| propertyPrefix == "pdfaSchema")`; absence is allowed. |
| `PDFA1B-XMP-EXTENSION-SCHEMA-VALUE-TYPES-001` | `ISO 19005-1:2005:6.7.8:7` | §6.7.8 | partial/proxy | `isValueTypeValidSeq == true && (valueTypePrefix == null \|\| valueTypePrefix == "pdfaSchema")`; absence is allowed. |
| `PDFA1B-XMP-EXTENSION-PROPERTY-NAME-001` | `ISO 19005-1:2005:6.7.8:8` | §6.7.8 | partial/proxy | `isNameValidText == true && namePrefix == "pdfaProperty"`. |
| `PDFA1B-XMP-EXTENSION-PROPERTY-VALUE-TYPE-001` | `ISO 19005-1:2005:6.7.8:9` | §6.7.8 | partial/proxy | `isValueTypeValidText == true && isValueTypeDefined == true && valueTypePrefix == "pdfaProperty"`. |
| `PDFA1B-XMP-EXTENSION-PROPERTY-CATEGORY-001` | `ISO 19005-1:2005:6.7.8:10` | §6.7.8 | partial/proxy | `isCategoryValidText == true && (category == "external" \|\| category == "internal") && categoryPrefix == "pdfaProperty"`. |
| `PDFA1B-XMP-EXTENSION-PROPERTY-DESCRIPTION-001` | `ISO 19005-1:2005:6.7.8:11` | §6.7.8 | partial/proxy | `isDescriptionValidText == true && descriptionPrefix == "pdfaProperty"`. |
| `PDFA1B-XMP-EXTENSION-VALUE-TYPE-NAME-001` | `ISO 19005-1:2005:6.7.8:12` | §6.7.8 | partial/proxy | `isTypeValidText == true && typePrefix == "pdfaType"`. |
| `PDFA1B-XMP-EXTENSION-VALUE-TYPE-NAMESPACE-001` | `ISO 19005-1:2005:6.7.8:13` | §6.7.8 | partial/proxy | `isNamespaceURIValidURI == true && namespaceURIPrefix == "pdfaType"`. |
| `PDFA1B-XMP-EXTENSION-VALUE-TYPE-PREFIX-001` | `ISO 19005-1:2005:6.7.8:14` | §6.7.8 | partial/proxy | `isPrefixValidText == true && prefixPrefix == "pdfaType"`. |
| `PDFA1B-XMP-EXTENSION-VALUE-TYPE-DESCRIPTION-001` | `ISO 19005-1:2005:6.7.8:15` | §6.7.8 | partial/proxy | `isDescriptionValidText == true && descriptionPrefix == "pdfaType"`. |
| `PDFA1B-XMP-EXTENSION-VALUE-TYPE-FIELDS-001` | `ISO 19005-1:2005:6.7.8:16` | §6.7.8 | partial/proxy | `isFieldValidSeq == true && (fieldPrefix == null \|\| fieldPrefix == "pdfaType")`; absence is allowed. |
| `PDFA1B-XMP-EXTENSION-FIELD-NAME-001` | `ISO 19005-1:2005:6.7.8:17` | §6.7.8 | partial/proxy | `isNameValidText == true && namePrefix == "pdfaField"`. |
| `PDFA1B-XMP-EXTENSION-FIELD-VALUE-TYPE-001` | `ISO 19005-1:2005:6.7.8:18` | §6.7.8 | partial/proxy | `isValueTypeValidText == true && isValueTypeDefined == true && valueTypePrefix == "pdfaField"`. |
| `PDFA1B-XMP-EXTENSION-FIELD-DESCRIPTION-001` | `ISO 19005-1:2005:6.7.8:19` | §6.7.8 | partial/proxy | `isDescriptionValidText == true && descriptionPrefix == "pdfaField"`. |
| `PDFA1B-ID-SCHEMA-001` | `ISO 19005-1:2005:6.7.11:1` | §6.7.11 | partial/proxy | Common packages agree; veraPDF recovery differs for invalid duplicate packages. |
| `PDFA1B-ID-PART-001` | `ISO 19005-1:2005:6.7.11:2` | §6.7.11 | partial/proxy | `part == 1`; common single-property packages agree. |
| `PDFA1B-ID-CONFORMANCE-001` | `ISO 19005-1:2005:6.7.11:3` | §6.7.11 | partial/proxy | `conformance == "B" \|\| conformance == "A"`; common single-property packages agree. |
| `PDFA1B-ID-PART-PREFIX-001` | `ISO 19005-1:2005:6.7.11:4` | §6.7.11 | partial/proxy | For the selected common identification property: `partPrefix == null \|\| partPrefix == "pdfaid"`; lexical prefixes are recovered from the XML source QName. |
| `PDFA1B-ID-CONFORMANCE-PREFIX-001` | `ISO 19005-1:2005:6.7.11:5` | §6.7.11 | partial/proxy | `conformancePrefix == null \|\| conformancePrefix == "pdfaid"` for the selected common property. |
| `PDFA1B-ID-AMD-PREFIX-001` | `ISO 19005-1:2005:6.7.11:6` | §6.7.11 | partial/proxy | `amdPrefix == null \|\| amdPrefix == "pdfaid"` for the selected common property; an absent `amd` is inapplicable and passes. |
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
| `PDFA1B-FONT-EMBEDDING-001` | `ISO 19005-1:2005:6.3.4:1` | §6.3.4 | partial/proxy | Implements `Subtype == "Type3" \|\| Subtype == "Type0" \|\| renderingMode == 3 \|\| containsFontFile == true` for bounded page/Form text-show paths, used Type0 descendants, annotation `/AP`/`/N` appearance streams (including every button Widget appearance state, not only the `/AS`-selected one), Pattern content streams, and Type3 `/CharProcs` glyph descriptions. `containsFontFile` for a `/FontFile2` stream is `valid_sfnt` via `ttf_parser::RawFace` (SFNT signature + table directory only), confirmed live: a font whose `maxp` table is malformed enough that a full `ttf_parser::Face::parse` fails still counts as embedded on veraPDF. |
| `PDFA1B-ICCBASED-001` | `ISO 19005-1:2005:6.2.3.2:1` | §6.2.3.2 | exact | For safely decoded profiles reached by the documented bounded content paths: `(deviceClass == "prtr" \|\| deviceClass == "mntr" \|\| deviceClass == "scnr" \|\| deviceClass == "spac") && (colorSpace == "RGB " \|\| colorSpace == "CMYK" \|\| colorSpace == "GRAY" \|\| colorSpace == "Lab ") && version < 3.0`. |
| `PDFA1B-ICCBASED-COMPONENTS-001` | `ISO 19005-1:2005:6.2.3.2:2` | §6.2.3.2 | exact | For safely decoded profiles reached by the documented bounded content paths: `N != null && ((N == 1 && colorSpace == "GRAY") \|\| (N == 3 && (colorSpace == "RGB " \|\| colorSpace == "Lab ")) \|\| (N == 4 && colorSpace == "CMYK"))`. |
| `PDFA1B-DEVICE-RGB-001` | `ISO 19005-1:2005:6.2.3.3:1` | §6.2.3.3 | exact | For DeviceRGB uses reached by the documented bounded content paths: `gOutputCS != null && gOutputCS == "RGB "`. |
| `PDFA1B-DEVICE-CMYK-001` | `ISO 19005-1:2005:6.2.3.3:2` | §6.2.3.3 | exact | For DeviceCMYK uses reached by the documented bounded content paths: `gOutputCS != null && gOutputCS == "CMYK"`. |
| `PDFA1B-DEVICE-GRAY-001` | `ISO 19005-1:2005:6.2.3.3:3` | §6.2.3.3 | exact | For DeviceGray uses reached by the documented bounded content paths: `gOutputCS != null`. |
| `PDFA1B-IMAGE-ALTERNATES-001` | `ISO 19005-1:2005:6.2.4:1` | §6.2.4 | exact | For invoked Image XObjects: `containsAlternates == false`. |
| `PDFA1B-XOBJECT-OPI-001` | `ISO 19005-1:2005:6.2.4:2` | §6.2.4 | exact | For invoked XObjects: `containsOPI == false`. |
| `PDFA1B-IMAGE-INTERPOLATE-001` | `ISO 19005-1:2005:6.2.4:3` | §6.2.4 | exact | For invoked Image XObjects: `Interpolate == false`. |
| `PDFA1B-IMAGE-BPC-001` | `ISO 19005-1:2005:6.2.4:4` | §6.2.4 | exact | For invoked Image XObjects: `isMask == true \|\| BitsPerComponent == null \|\| BitsPerComponent == 1 \|\| BitsPerComponent == 2 \|\| BitsPerComponent == 4 \|\| BitsPerComponent == 8`. |
| `PDFA1B-IMAGE-MASK-BPC-001` | `ISO 19005-1:2005:6.2.4:5` | §6.2.4 | exact | For images reached through another image's `/Mask` reference: `BitsPerComponent == null \|\| BitsPerComponent == 1`. |
| `PDFA1B-FORM-POSTSCRIPT-001` | `ISO 19005-1:2005:6.2.5:1` | §6.2.5 | exact | For invoked Form XObjects: `(Subtype2 == null \|\| Subtype2 != "PS") && containsPS == false`. |
| `PDFA1B-FORM-REFERENCE-001` | `ISO 19005-1:2005:6.2.6:1` | §6.2.6 | exact | For invoked Form XObjects: `containsRef == false`. |
| `PDFA1B-XOBJECT-POSTSCRIPT-001` | `ISO 19005-1:2005:6.2.7:1` | §6.2.7 | exact | For invoked XObjects: `Subtype != "PS"`. |
| `PDFA1B-EXTGSTATE-TR-001` | `ISO 19005-1:2005:6.2.8:1` | §6.2.8 | exact | For indirect ExtGState dictionaries selected by executed `gs` operators: `containsTR == false`. |
| `PDFA1B-EXTGSTATE-TR2-001` | `ISO 19005-1:2005:6.2.8:2` | §6.2.8 | exact | For indirect ExtGState dictionaries selected by executed `gs` operators: `containsTR2 == false \|\| TR2NameValue == "Default"`. |
| `PDFA1B-RENDERING-INTENT-001` | `ISO 19005-1:2005:6.2.9:1` | §6.2.9 | exact | For rendering intents reached through executed `ri` and `gs` operators or invoked Image XObjects: the name is `RelativeColorimetric`, `AbsoluteColorimetric`, `Perceptual`, or `Saturation`. |
| `PDFA1B-CONTENT-OPERATOR-001` | `ISO 19005-1:2005:6.2.10:1` | §6.2.10 | exact | No undefined operator occurs in bounded executed page or Form content, including compatibility sections delimited by `BX` and `EX`. |
| `PDFA1B-FONT-TYPE-001` | `ISO 19005-1:2005:6.3.2:1` | §6.3.2 | exact | For fonts reached by bounded text-show paths: `Type == "Font"`. Confirmed live: a font with a valid `/Subtype` but a missing `/Type` fails on both veraPDF and locally, with the failure attached to the same font object. |
| `PDFA1B-FONT-SUBTYPE-001` | `ISO 19005-1:2005:6.3.2:2` | §6.3.2 | exact | Modeled fonts have subtype `Type1`, `MMType1`, `TrueType`, `Type3`, `Type0`, `CIDFontType0`, or `CIDFontType2`; missing/unsupported subtypes create no veraPDF `PDFont` object and are inapplicable, so this predicate cannot fail on either side by construction (confirmed live: a font with an unsupported `/Subtype`, used normally, is fully compliant on both). `/Subtype` itself is resolved through indirection before this recognition check (confirmed live: an indirect reference to a supported name, e.g. `/TrueType`, is recognized exactly like a direct one, so the font is not wrongly treated as inapplicable and skipped). |
| `PDFA1B-FONT-BASEFONT-001` | `ISO 19005-1:2005:6.3.2:3` | §6.3.2 | exact | For fonts reached by bounded text-show paths: `Subtype == "Type3" \|\| fontName != null`. Confirmed live: missing and wrong-type `/BaseFont` both fail on veraPDF and locally. |
| `PDFA1B-FONT-FIRSTCHAR-001` | `ISO 19005-1:2005:6.3.2:4` | §6.3.2 | exact | For modeled simple fonts: `isStandard == true \|\| FirstChar != null`. `isStandard` (the standard-14-fonts exemption) is confirmed live to apply only to `Type1`/`MMType1`; a `TrueType` or `Type3` font whose `/BaseFont` matches a standard-14 name is not exempt. |
| `PDFA1B-FONT-LASTCHAR-001` | `ISO 19005-1:2005:6.3.2:5` | §6.3.2 | exact | For modeled simple fonts: `isStandard == true \|\| LastChar != null`. `isStandard` is confirmed live to apply only to `Type1`/`MMType1`, not `TrueType`/`Type3`. |
| `PDFA1B-FONT-WIDTHS-001` | `ISO 19005-1:2005:6.3.2:6` | §6.3.2 | exact | For modeled simple fonts: `isStandard == true \|\| (Widths_size != null && Widths_size == LastChar - FirstChar + 1)`. `isStandard` is confirmed live to apply only to `Type1`/`MMType1`, not `TrueType`/`Type3`. `/Widths` is resolved through indirection before its length is read (confirmed live: an indirect reference to a correctly sized array is accepted exactly like a direct one). |
| `PDFA1B-FONT-FILE-SUBTYPE-001` | `ISO 19005-1:2005:6.3.2:7` | §6.3.2 | exact | For embedded font streams reached through modeled fonts: `fontFileSubtype == null \|\| fontFileSubtype == "Type1C" \|\| fontFileSubtype == "CIDFontType0C"`. Confirmed live for both a simple font's own `/FontDescriptor` and a Type0 font's first descendant's `/FontDescriptor` (the recursive `record_font` call applies the same dictionary check to the descendant). |
| `PDFA1B-TYPE0-CID-SYSTEM-INFO-001` | `ISO 19005-1:2005:6.3.3.1:1` | §6.3.3.1 | partial/proxy | `cmapName == "Identity-H" \|\| cmapName == "Identity-V" \|\| (CIDFontOrdering != null && CIDFontOrdering == CMapOrdering && CIDFontRegistry != null && CIDFontRegistry == CMapRegistry)` over embedded CMaps reached by bounded text-show paths. `/Encoding` is resolved through indirection before the Identity-H/V name check (confirmed live: an *indirect* reference to `/Identity-H` is exempt exactly like a direct one). `CIDFontRegistry`/`CIDFontOrdering`/`CMapRegistry`/`CMapOrdering` are each resolved through indirection before comparison (confirmed live: an indirect `/Registry` string compares equal to a matching direct one) -- still `partial/proxy` because the predefined (non-embedded, named) CMap collection is not modeled, per `resolve_cmap_decoder`'s own doc comment. |
| `PDFA1B-CIDTOGIDMAP-001` | `ISO 19005-1:2005:6.3.3.2:1` | §6.3.3.2 | exact | For used first descendants: `Subtype != "CIDFontType2" \|\| CIDToGIDMap != null \|\| renderingMode == 3`; a modeled map is `Identity` or a stream, resolved through indirection first (confirmed live: an *indirect* reference to the name `/Identity` is accepted exactly like a direct one). |
| `PDFA1B-CMAP-EMBEDDING-001` | `ISO 19005-1:2005:6.3.3.3:1` | §6.3.3.3 | partial/proxy | For Type0 fonts reached by bounded text-show paths: `CMapName == "Identity-H" \|\| CMapName == "Identity-V" \|\| containsEmbeddedFile == true`. `/Encoding` is resolved through indirection before the Identity-H/V name check (confirmed live: an *indirect* reference to `/Identity-H` is exempt exactly like a direct one). |
| `PDFA1B-CMAP-WMODE-001` | `ISO 19005-1:2005:6.3.3.3:2` | §6.3.3.3 | exact | For bounded embedded CMaps: the parsed stream `WMode` equals the stream-dictionary `WMode`, defaulting each missing value to zero, resolved through indirection first (confirmed live: an *indirect* `/WMode` value is resolved and compared exactly like a direct one). |
| `PDFA1B-CMAP-CID-RANGE-001` | none | none | none | Local bounded precondition supporting the §6.3.5 test-1 glyph-presence predicate below: a CID a rendered byte decodes to must be within the descendant CIDFont's supported range before glyph presence can be meaningfully evaluated. Not an independently numbered veraPDF test. |
| `PDFA1B-CMAP-MAX-CID-001` | `ISO 19005-1:2005:6.1.12:10` | §6.1.12 | exact | Every embedded CMap's maximum CID is at most 65,535. |
| `PDFA1B-TYPE1-GLYPH-PRESENCE-001` | `ISO 19005-1:2005:6.3.5:1` | §6.3.5 | partial/proxy | For simple Type1, MMType1, and Type1C fonts reached by bounded text-show paths: a bounded embedded program defines a glyph for every rendered byte. |
| `PDFA1B-TRUETYPE-GLYPH-PRESENCE-001` | `ISO 19005-1:2005:6.3.5:1` | §6.3.5 | partial/proxy | For TrueType, `CIDFontType2`, and CID-keyed CFF (`CIDFontType0C`) fonts reached by bounded text-show paths: a bounded embedded program defines a glyph for every rendered byte or CID. For `CIDFontType2`, `/CIDToGIDMap` is resolved through indirection before the `/Identity` name check (confirmed live via the paired `PDFA1B-TRUETYPE-GLYPH-WIDTH-001` fix); an unresolvable map is treated as inapplicable (silently skipped) rather than a missing-glyph failure, since `PDFA1B-CIDTOGIDMAP-001` already flags a genuinely missing/invalid map separately. |
| `PDFA1B-TYPE1-SUBSET-CHARSET-001` | `ISO 19005-1:2005:6.3.5:2` | §6.3.5 | partial/proxy | For a subset Type1/MMType1 font (`BaseFont` with a six-uppercase-letter subset tag): the descriptor `/CharSet` string names every rendered glyph. |
| `PDFA1B-CID-SUBSET-CIDSET-001` | `ISO 19005-1:2005:6.3.5:3` | §6.3.5 | partial/proxy | For a subset `CIDFontType0`/`CIDFontType2` descendant: the descriptor `/CIDSet` stream's bits identify every rendered CID as present. |
| `PDFA1B-TRUETYPE-GLYPH-WIDTH-001` | `ISO 19005-1:2005:6.3.6:1` | §6.3.6 | partial/proxy | For TrueType, `CIDFontType2`, Type1, and Type1C/`CIDFontType0C` fonts reached by bounded text-show paths: a rendered byte or CID's width in the embedded program agrees with its dictionary-declared width within 1 unit, matching veraPDF's `abs(...) <= 1` tolerance. For `CIDFontType2`, `/CIDToGIDMap` is resolved through indirection before the `/Identity` name check (confirmed live: an indirect `/Identity` reference resolves rendered CIDs to the same glyphs as a direct one, so a genuine width mismatch is still caught rather than silently skipped). For simple fonts, both the whole `/Widths` array and each individual entry it reads are resolved through indirection before comparison (confirmed live for an indirect array and, separately, for a direct array whose one entry is an indirect reference to a mismatched value -- the mismatch is still caught rather than silently skipped). A descendant's own `/Subtype` (the `CIDFontType0`/`CIDFontType2` selector) is resolved through indirection before this and the paired presence check run at all (confirmed live: an indirect reference to `/CIDFontType2` is recognized exactly like a direct one, so a genuine width mismatch is still caught rather than the descendant being treated as an unrecognized subtype and silently skipped). For CID fonts, `/DW` and each `/W` singles-group width entry are likewise resolved through indirection before comparison (confirmed live for both). |
| `PDFA1B-TRUETYPE-NONSYMBOLIC-ENCODING-001` | `ISO 19005-1:2005:6.3.7:1` | §6.3.7 | partial/proxy | For used TrueType fonts whose descriptor Flags do not set Symbolic: `(Encoding == "MacRomanEncoding" \|\| Encoding == "WinAnsiEncoding") && containsDifferences == false`. |
| `PDFA1B-TRUETYPE-SYMBOLIC-ENCODING-001` | `ISO 19005-1:2005:6.3.7:2` | §6.3.7 | partial/proxy | For used TrueType fonts whose descriptor Flags set Symbolic: `Encoding == null`. |
| `PDFA1B-TRUETYPE-SYMBOLIC-CMAP-001` | `ISO 19005-1:2005:6.3.7:3` | §6.3.7 | exact | For recognized embedded symbolic TrueType programs: `nrCmaps == 1`, read from the bounded SFNT `cmap` table header via `ttf_parser::RawFace` (confirmed live: this holds even when the rest of the font -- `maxp`, `hhea`, ... -- is malformed enough that a full `ttf_parser::Face::parse` fails). |
| `PDFA1B-EXTGSTATE-SMASK-001` | `ISO 19005-1:2005:6.4:1` | §6.4 | partial/proxy | For indirect ExtGState dictionaries selected by executed `gs`: `containsSMask == false \|\| SMaskNameValue == "None"`. |
| `PDFA1B-XOBJECT-SMASK-001` | `ISO 19005-1:2005:6.4:2` | §6.4 | partial/proxy | For XObjects in the bounded invoked graph: `containsSMask == false`. |
| `PDFA1B-TRANSPARENCY-GROUP-001` | `ISO 19005-1:2005:6.4:3` | §6.4 | partial/proxy | For page Groups and bounded invoked-Form Groups: `S != "Transparency"`. |
| `PDFA1B-EXTGSTATE-BLEND-MODE-001` | `ISO 19005-1:2005:6.4:4` | §6.4 | partial/proxy | For indirect ExtGState dictionaries selected by executed `gs`: `containsBM == false \|\| BMNameValue == "Normal" \|\| BMNameValue == "Compatible"`. |
| `PDFA1B-EXTGSTATE-STROKE-ALPHA-001` | `ISO 19005-1:2005:6.4:5` | §6.4 | partial/proxy | For modeled numeric values in used indirect ExtGState dictionaries: `CA == null \|\| abs(CA - 1.0) < 0.000001`. |
| `PDFA1B-EXTGSTATE-FILL-ALPHA-001` | `ISO 19005-1:2005:6.4:6` | §6.4 | partial/proxy | For modeled numeric values in used indirect ExtGState dictionaries: `ca == null \|\| abs(ca - 1.0) < 0.000001`. |
| `PDFA1B-ANNOTATION-SUBTYPE-001` | `ISO 19005-1:2005:6.5.2:1` | §6.5.2 | partial/proxy | For page `/Annots` entries resolving to dictionaries: `Subtype` is `Text`, `Link`, `FreeText`, `Line`, `Square`, `Circle`, `Highlight`, `Underline`, `Squiggly`, `StrikeOut`, `Stamp`, `Ink`, `Popup`, `Widget`, `PrinterMark`, or `TrapNet`. |
| `PDFA1B-ANNOTATION-OPACITY-001` | `ISO 19005-1:2005:6.5.3:1` | §6.5.3 | partial/proxy | For modeled numeric annotation values: `CA == null \|\| CA == 1.0`. |
| `PDFA1B-ANNOTATION-FLAGS-001` | `ISO 19005-1:2005:6.5.3:2` | §6.5.3 | partial/proxy | `F != null && (F & 4) == 4 && (F & 1) == 0 && (F & 2) == 0 && (F & 32) == 0`. |
| `PDFA1B-ANNOTATION-COLOR-001` | `ISO 19005-1:2005:6.5.3:3` | §6.5.3 | partial/proxy | `(containsC == false && containsIC == false) \|\| gOutputCS == "RGB "`, using the pinned PDF/A-1 output-colour fold. |
| `PDFA1B-ANNOTATION-AP-ENTRIES-001` | `ISO 19005-1:2005:6.5.3:4` | §6.5.3 | partial/proxy | A resolved annotation appearance dictionary is absent or contains exactly the `N` key. |
| `PDFA1B-WIDGET-BUTTON-APPEARANCE-001` | `ISO 19005-1:2005:6.5.3:5` | §6.5.3 | partial/proxy | When `AP == "N"`, `Subtype == "Widget"`, and direct or inherited `FT == "Btn"`, `N` is a nonempty appearance subdictionary. |
| `PDFA1B-ANNOTATION-NORMAL-APPEARANCE-001` | `ISO 19005-1:2005:6.5.3:6` | §6.5.3 | partial/proxy | When `AP == "N"` and the annotation is not a button Widget, `N` resolves to an appearance stream. |
| `PDFA1B-ACTION-TYPE-001` | `ISO 19005-1:2005:6.6.1:1` | §6.6.1 | partial/proxy | For actions reached through the bounded pinned-model graph: `S == "GoTo" \|\| S == "GoToR" \|\| S == "Thread" \|\| S == "URI" \|\| S == "Named" \|\| S == "SubmitForm"`. |
| `PDFA1B-NAMED-ACTION-001` | `ISO 19005-1:2005:6.6.1:2` | §6.6.1 | partial/proxy | For reached actions with `S == "Named"`: `N == "NextPage" \|\| N == "PrevPage" \|\| N == "FirstPage" \|\| N == "LastPage"`. |
| `PDFA1B-WIDGET-ACTION-001` | `ISO 19005-1:2005:6.6.1:3` | §6.6.1 | partial/proxy | For Widget annotations reached from page `/Annots`: `containsA == false`; key presence is tested regardless of value type. |
| `PDFA1B-WIDGET-ADDITIONAL-ACTIONS-001` | `ISO 19005-1:2005:6.6.2:1` | §6.6.2 | partial/proxy | For Widget annotations reached from page `/Annots`: `containsAA == false`; key presence is tested regardless of value type. |
| `PDFA1B-FIELD-ADDITIONAL-ACTIONS-001` | `ISO 19005-1:2005:6.6.2:2` | §6.6.2 | partial/proxy | For dictionaries directly in AcroForm `/Fields` and named field descendants reached through `/Kids`: `containsAA == false`. |
| `PDFA1B-CATALOG-ADDITIONAL-ACTIONS-001` | `ISO 19005-1:2005:6.6.2:3` | §6.6.2 | exact | `containsAA == false` on the document catalog; key presence is tested regardless of value type. |
| `PDFA1B-ACROFORM-NEED-APPEARANCES-001` | `ISO 19005-1:2005:6.9:1` | §6.9 | exact | For a modeled catalog AcroForm: `NeedAppearances == null \|\| NeedAppearances == false`; a present non-boolean value follows veraPDF's conservative true fallback and fails. |
| `PDFA1B-WIDGET-APPEARANCE-001` | `ISO 19005-1:2005:6.9:2` | §6.9 | partial/proxy | For Widget annotations reached from page `/Annots`: `AP != null`; veraPDF exposes AP only when it resolves to a dictionary, so streams and scalar values fail while an empty dictionary passes. |
| `PDFA1B-NAMES-EMBEDDED-FILES-001` | `ISO 19005-1:2005:6.1.11:2` | §6.1.11 | exact | For the catalog's dictionary-based `/Names`: `EmbeddedFiles` is absent; direct null is empty, while an indirect null remains a present key. |
| `PDFA1B-OPTIONAL-CONTENT-001` | `ISO 19005-1:2005:6.1.13:1` | §6.1.13 | exact | On the document catalog: `OCProperties` is empty or absent; direct null is empty, while any non-empty value, including an indirect null, is present. |
| `PDFA1B-FILE-SPEC-EMBEDDED-FILE-001` | `ISO 19005-1:2005:6.1.11:1` | §6.1.11 | exact | `containsEF == false` for every reachable file specification: dictionary-based values in the catalog's `EmbeddedFiles` name tree (`Names` arrays and recursive `Kids` traversed with configured reference-depth bounds), and a `GoToR`/`SubmitForm` action's `/F` entry (confirmed against veraPDF 1.28.2, which creates the same `CosFileSpecification` object either way). |
| `PDFA1B-STREAM-EXTERNAL-DATA-001` | `ISO 19005-1:2005:6.1.7:3` | §6.1.7 | exact | For every selected indirect stream dictionary: `F == null && FFilter == null && FDecodeParms == null`; direct null and last-duplicate-key selection follow the pinned COS model. |
| `PDFA1B-STREAM-LZW-001` | `ISO 19005-1:2005:6.1.10:1` | §6.1.10 | exact | For every selected indirect stream filter: `internalRepresentation != "LZWDecode"`; escaped names are decoded and direct, array, and indirect declarations are resolved. |
| `PDFA1B-INLINE-IMAGE-LZW-001` | `ISO 19005-1:2005:6.1.10:2` | §6.1.10 | exact | No inline image in bounded executed page/Form content declares `internalRepresentation == "LZWDecode"` (or the inline-image-only `LZW` spelling). |
| `PDFA1B-INTEGER-RANGE-001` | `ISO 19005-1:2005:6.1.12:1` | §6.1.12 | exact | Every integer in the active revision's effective object graph is evaluated from its source token before `lopdf` narrowing; overwritten duplicate values and superseded revisions are inapplicable. |
| `PDFA1B-REAL-RANGE-001` | `ISO 19005-1:2005:6.1.12:2` | §6.1.12 | exact | Every real in the active revision's effective object graph is evaluated from its source token before `lopdf` `f32` normalization. |
| `PDFA1B-STRING-LENGTH-001` | `ISO 19005-1:2005:6.1.12:3` | §6.1.12 | exact | Every effective literal or hexadecimal string value has fewer than 65,536 decoded bytes; original escape and odd-nibble forms are retained separately. |
| `PDFA1B-NAME-LENGTH-001` | `ISO 19005-1:2005:6.1.12:4` | §6.1.12 | exact | Every effective name value has at most 127 decoded bytes after `#xx` processing; dictionary keys are outside the pinned `CosName` model. |
| `PDFA1B-ARRAY-LENGTH-001` | `ISO 19005-1:2005:6.1.12:5` | §6.1.12 | exact | Every parsed array has at most 8,191 entries. |
| `PDFA1B-DICTIONARY-LENGTH-001` | `ISO 19005-1:2005:6.1.12:6` | §6.1.12 | exact | Every effective dictionary has at most 4,095 unique non-null entries after last-duplicate-key selection; direct null entries are absent in the pinned COS size. |
| `PDFA1B-INDIRECT-OBJECT-COUNT-001` | `ISO 19005-1:2005:6.1.12:7` | §6.1.12 | exact | Active non-free classic or compressed xref entries supply `nrIndirects <= 8388607`; configured input/object limits may fail operationally before a larger source is modeled. |
| `PDFA1B-GRAPHICS-STATE-NESTING-001` | `ISO 19005-1:2005:6.1.12:8` | §6.1.12 | exact | Bounded invoked page/Form content maintains at most 28 saved graphics states; Form invocation starts a separate graphics-state stack. |
| `PDFA1B-DEVICEN-COMPONENTS-001` | `ISO 19005-1:2005:6.1.12:9` | §6.1.12 | exact | Every DeviceN colour space array reached anywhere in the parsed object graph, or through the bounded content-execution paths, declares at most 8 components. |
