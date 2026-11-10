## Dev

- **Fix**: Decode Type 1 byte-255 glyph widths as signed integers while preserving Type 2 fixed-point decoding [#635](https://github.com/JosephBARBIERDARNAL/page/issues/635)
- **Fix**: Validate applicable XMP rules in non-catalog metadata streams and reuse catalog extension declarations [#634](https://github.com/JosephBARBIERDARNAL/page/issues/634)
- **Fix**: Respect explicit image `/ColorSpace` overrides for JPEG2000 colour specifications while retaining channel and bit-depth checks [#633](https://github.com/JosephBARBIERDARNAL/page/issues/633)
- **Chore**: Update the veraPDF reference validator version to 1.30.4
- **Fix**: Recognize stream terminators with a missing preceding EOL when indexing raw PDF objects
- **Fix**: Match veraPDF metadata rule applicability and classify short table rows without extra row-span failures

## 0.13.0

- **Fix**: Validate stream `/Length` independently from the required EOL before `endstream` [#631](https://github.com/JosephBARBIERDARNAL/page/issues/631)
- **Fix**: Recognize Unicode mappings from used Type 1 `/Differences` glyph names [#630](https://github.com/JosephBARBIERDARNAL/page/issues/630)
- **Fix**: Preserve Type 1 embedded-encoding and standard `/CharSet` Unicode mapping exceptions [#630](https://github.com/JosephBARBIERDARNAL/page/issues/630)
- **Fix**: Require the PDF/UA Microsoft Unicode cmap for non-symbolic TrueType fonts only when `/Differences` is present [#629](https://github.com/JosephBARBIERDARNAL/page/issues/629)
- **Fix**: Parse array destinations and compact hex strings in ToUnicode CMaps [#628](https://github.com/JosephBARBIERDARNAL/page/issues/628)
- **Fix**: Compare PDF/A-2 and PDF/A-3 `/CIDSet` declarations with the complete embedded CID subset [#627](https://github.com/JosephBARBIERDARNAL/page/issues/627)
- **Fix**: Compare PDF/A-2 and PDF/A-3 Type 1 `/CharSet` values with every glyph in the embedded subset [#626](https://github.com/JosephBARBIERDARNAL/page/issues/626)
- **Fix**: Apply the CIDFont default width of 1000 for absent `/DW` entries when checking embedded glyph widths [#625](https://github.com/JosephBARBIERDARNAL/page/issues/625)
- **Fix**: Respect TrueType cmap encodings when resolving simple-font glyphs and checking their widths [#624](https://github.com/JosephBARBIERDARNAL/page/issues/624)
- **Fix**: Report PDF/A-1 Info date mismatches when XMP metadata or the corresponding date property is missing [#623](https://github.com/JosephBARBIERDARNAL/page/issues/623)
- **Doc**: Refresh CLI examples, output, and command references across the README and documentation [#619](https://github.com/JosephBARBIERDARNAL/page/issues/619)
- **Fix**: Align detailed CLI output tests and documentation with the removal of the `(example finding)` label [#619](https://github.com/JosephBARBIERDARNAL/page/issues/619)
- **Fix**: Include page numbers for annotations, content streams, and used resources, including shared objects and resource dependencies [#619](https://github.com/JosephBARBIERDARNAL/page/issues/619)
- **Fix**: Limit detailed CLI reports to three example locations per finding, with counts and `--locations all` for exhaustive output [#619](https://github.com/JosephBARBIERDARNAL/page/issues/619)
- **Feat**: Include object locations and page numbers for page objects in detailed reports, JSON, and language bindings [#619](https://github.com/JosephBARBIERDARNAL/page/issues/619)
- **Fix**: Keep detailed CLI failure descriptions on the ISO rule ID line
- **Refactor**: Remove possible causes and suggested fixes from core Rust reports, CLI output, JSON, and Python and JavaScript bindings

### 0.12.2

- **Chore**: Preflight CLI and package artifacts before tagging a release [#620](https://github.com/JosephBARBIERDARNAL/page/issues/620)

### 0.12.1

- **Fix**: Release issue

## 0.12.0

- **Fix**: Match validation guidance to exact ISO test identifiers
- **Feat**: Add possible causes and suggested fixes to validation failures and expose them in JSON, language bindings, and detailed CLI output [#33](https://github.com/JosephBARBIERDARNAL/page/issues/33)
- **Refactor**: Emit ISO rule identifiers directly from validation checks and remove the local rule-ID mapping table
- **Doc**: Attribute ISO clause identifiers sourced from the veraPDF validation profiles
- **Fix**: Assign ISO clause identifiers to previously uncovered PDF/A checks
- **Fix**: Limit the standard structure-tag remapping finding to PDF/A-2 and PDF/A-3, which define that check
- **Fix**: Recognize both supported Type 1 font program headers
- **Fix**: Return an unusable PDF trailer `/Root` as a parser error
- **Feat**: Use ISO specification rule IDs in validation reports
- **Fix**: Allow filtered metadata streams in PDF/A-2 and PDF/A-3
- **Fix**: Require catalog language when XMP alternatives provide no natural language [#610](https://github.com/JosephBARBIERDARNAL/page/issues/610)
- **Fix**: Recognize matching embedded `/UseCMap` streams [#612](https://github.com/JosephBARBIERDARNAL/page/issues/612)
- **Fix**: Report unembedded non-predefined CMaps referenced through `/UseCMap` [#612](https://github.com/JosephBARBIERDARNAL/page/issues/612)
- **Fix**: Report missing CIDSystemInfo checks for unembedded non-predefined CMaps [#611](https://github.com/JosephBARBIERDARNAL/page/issues/611)
- **Fix**: Report missing alternate text for Link annotations [#609](https://github.com/JosephBARBIERDARNAL/page/issues/609)
- **Fix**: Color only rule IDs red in CLI details output
- **Fix**: Remove object IDs from validation failure reports and output

## 0.11.0

- **Doc**: Add guide on lazy validation [#377](https://github.com/JosephBARBIERDARNAL/page/issues/377)
- **Feat**: Implement `Deserialize` for Rust JSON report types [#403](https://github.com/JosephBARBIERDARNAL/page/issues/403)
- **Refactor**: Make Rust `SafetyLimits` fields private and use getters with chainable `with_` setters [#400](https://github.com/JosephBARBIERDARNAL/page/issues/400)
- **Feat**: Add `ValidationReport.to_dict()` and accept bytes-like inputs in the Python byte APIs [#402](https://github.com/JosephBARBIERDARNAL/page/issues/402)
- **Refactor**: Remove the no-op `initialize()` function from the Wasm API [#398](https://github.com/JosephBARBIERDARNAL/page/issues/398)
- **Fix**: Expose failure object IDs and categories on the Wasm TypeScript report [#390](https://github.com/JosephBARBIERDARNAL/page/issues/390)
- **Fix**: Align JSON report field names with Rust, Python, and Wasm report attributes [#389](https://github.com/JosephBARBIERDARNAL/page/issues/389)
- **Refactor**: Rename the top-level validation error to `PageError` across Rust, Python, and Wasm APIs [#401](https://github.com/JosephBARBIERDARNAL/page/issues/401)

## 0.10.0

- **Fix**: Give CLI usage errors exit code 1 and document stable CLI exit codes [#394](https://github.com/JosephBARBIERDARNAL/page/issues/394)
- **Fix**: Report the PDF/A-1 indirect-object limit as a conformance failure across API surfaces [#387](https://github.com/JosephBARBIERDARNAL/page/issues/387)
- **Refactor**: Remove unimplemented Rust validation profiles and their rejection helpers, and simplify Python profile enums to string values without `is_implemented` [#388](https://github.com/JosephBARBIERDARNAL/page/issues/388)
- **Refactor**: Move validation error exit-code mapping into the CLI and keep the PDF/A-1 indirect-object conformance constant private to the validation crate [#392](https://github.com/JosephBARBIERDARNAL/page/issues/392)
- **Refactor**: Hide the CLI validation exit-code helper from the published API documentation [#393](https://github.com/JosephBARBIERDARNAL/page/issues/393)
- **Fix**: Use shared `Display` formatting for failure categories and failures in Rust reports and CLI details output [#396](https://github.com/JosephBARBIERDARNAL/page/issues/396)
- **Refactor**: Limit the public Rust document model to summary fields, hide metadata and parsing types, and always include a document summary in Rust and Python validation reports [#386](https://github.com/JosephBARBIERDARNAL/page/issues/386)
- **Doc**: Fix the duplicate logo in the mobile navigation drawer [#385](https://github.com/JosephBARBIERDARNAL/page/issues/385)
- **Fix**: Export safety-limit defaults from Rust to the Wasm TypeScript wrapper [#362](https://github.com/JosephBARBIERDARNAL/page/issues/362)
- **Perf**: Avoid copying Python bytes inputs during validation [#371](https://github.com/JosephBARBIERDARNAL/page/issues/371)
- **Feat**: Expose the source path on Python validation reports [#370](https://github.com/JosephBARBIERDARNAL/page/issues/370)
- **Refactor**: Hide CLI presentation helpers from the published API documentation [#368](https://github.com/JosephBARBIERDARNAL/page/issues/368)
- **Feat**: Allow selecting summary output explicitly from the CLI [#364](https://github.com/JosephBARBIERDARNAL/page/issues/364)
- **Feat**: Include failure categories and attributed object IDs in JSON reports [#358](https://github.com/JosephBARBIERDARNAL/page/issues/358)

## 0.9.0

- **Feat**: Expose Python validation profiles and failure categories as standard enums and accept profile strings [#369](https://github.com/JosephBARBIERDARNAL/page/issues/369)
- **Feat**: Expose typed validation errors in the Rust, Python, and Wasm APIs [#355](https://github.com/JosephBARBIERDARNAL/page/issues/355)
- **Fix**: Use the correct validation family in report headings [#354](https://github.com/JosephBARBIERDARNAL/page/issues/354)
- **Refactor**: Remove duplicate snake_case and `init` aliases from the Wasm API [#373](https://github.com/JosephBARBIERDARNAL/page/issues/373)
- **Refactor**: Keep validation report exit-code mapping in the CLI [#361](https://github.com/JosephBARBIERDARNAL/page/issues/361)
- **Fix**: Hide unimplemented validation profiles from the CLI and language bindings [#360](https://github.com/JosephBARBIERDARNAL/page/issues/360)
- **Feat**: Add canonical parsing and string names for validation profiles [#359](https://github.com/JosephBARBIERDARNAL/page/issues/359)
- **Fix**: Use one stable JSON report schema across the CLI, Python, and Wasm bindings [#356](https://github.com/JosephBARBIERDARNAL/page/issues/356)
- **Refactor**: Keep terminal validation failures in `Result` errors and limit report failures to metadata and conformance findings [#349](https://github.com/JosephBARBIERDARNAL/page/issues/349)
- **Refactor**: Mark public Rust models and enums as non-exhaustive and add chainable safety-limit setters for future API extensions [#344](https://github.com/JosephBARBIERDARNAL/page/issues/344)
- **Refactor**: Make validation entry points extensible with a Rust `ValidationOptions` value, keyword-only Python arguments, and a TypeScript options object; rename the first-failure mode to lazy and remove `validate_pdf_fast` / `validate_pdf_bytes_fast` from the public Rust API [#347](https://github.com/JosephBARBIERDARNAL/page/issues/347)
- **Refactor**: Keep parser errors behind a standard error source in the public Rust API [#346](https://github.com/JosephBARBIERDARNAL/page/issues/346)
- **Doc**: Add a per-file overview to every core validation Rust source file [#331](https://github.com/JosephBARBIERDARNAL/page/issues/331)
- **Feat**: Disable all safety limits with a CLI flag or an unlimited preset [#342](https://github.com/JosephBARBIERDARNAL/page/issues/342)
- **Refactor**: Remove the legacy `PdfDocument::output_intents` field; use `output_intents_summary` instead [#345](https://github.com/JosephBARBIERDARNAL/page/issues/345)

## 0.8.0

- **Fix**: Bound aggregate decoded font-stream retention [#336](https://github.com/JosephBARBIERDARNAL/page/issues/336)
- **Fix**: Propagate safety-limit errors while inspecting embedded PDFs [#338](https://github.com/JosephBARBIERDARNAL/page/issues/338)
- **Fix**: Bound repeated execution of shared Form XObjects [#335](https://github.com/JosephBARBIERDARNAL/page/issues/335)
- **Perf**: Add a short real-world PDF regression benchmark to CI [#341](https://github.com/JosephBARBIERDARNAL/page/issues/341)
- **Fix**: Include generated API files in the published WASM package [#334](https://github.com/JosephBARBIERDARNAL/page/issues/334)
- **Refactor**: Remove the obsolete validation report status field
- **Refactor**: Replace `ttf-parser` with `read-fonts` for embedded font inspection [#36](https://github.com/JosephBARBIERDARNAL/page/issues/36)
- **Feat**: Report distinct failed rules and raw failed checks [#306](https://github.com/JosephBARBIERDARNAL/page/issues/306)
- **Perf**: Consolidate raw syntax and stream scans [#328](https://github.com/JosephBARBIERDARNAL/page/issues/328)
- **Perf**: Cache font usages, shown bytes, CIDs, and embedded font streams [#327](https://github.com/JosephBARBIERDARNAL/page/issues/327)
- **Perf**: Defer whole-document font summaries on the lazy validation path [#326](https://github.com/JosephBARBIERDARNAL/page/issues/326)
- **Doc**: Add a dedicated page for safety limits [#320](https://github.com/JosephBARBIERDARNAL/page/issues/320)
- **Perf**: Expand the benchmark with profile comparisons and a feature-heavy fixture
- **Perf**: Make document feature inspection profile-demand driven [#325](https://github.com/JosephBARBIERDARNAL/page/issues/325)
- **Perf**: Inferred-profile summary mode now uses lazy validation [#324](https://github.com/JosephBARBIERDARNAL/page/issues/324)

## 0.7.0

- **Fix**: Use the correct CLI error message across all consumers [#312](https://github.com/JosephBARBIERDARNAL/page/issues/312)
- **Fix**: ToUnicode CMap ranges lack a cumulative expansion limit [#318](https://github.com/JosephBARBIERDARNAL/page/issues/318)
- **Fix**: Tagged-table spans can cause unbounded grid allocations [#317](https://github.com/JosephBARBIERDARNAL/page/issues/317)
- **Fix**: Font stream decode fallback bypasses max_decoded_stream_size [#316](https://github.com/JosephBARBIERDARNAL/page/issues/316)
- **Fix**: Encrypted PDFs can bypass the configured object-count limit [#315](https://github.com/JosephBARBIERDARNAL/page/issues/315)
- **Doc**: Improve the landing page [#282](https://github.com/JosephBARBIERDARNAL/page/issues/282)
- **Doc**: Add a changelog page [#274](https://github.com/JosephBARBIERDARNAL/page/issues/274)
