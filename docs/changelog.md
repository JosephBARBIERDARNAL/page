## Dev

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
- **Doc**: Add page dedicated to safety limits [#320](https://github.com/JosephBARBIERDARNAL/page/issues/320)
- **Perf**: Expand the benchmark with profile comparisons and a feature-heavy fixture
- **Perf**: Make document feature inspection profile-demand driven [#325](https://github.com/JosephBARBIERDARNAL/page/issues/325)
- **Perf**: Inferred-profile summary mode now uses lazy validation [#324](https://github.com/JosephBARBIERDARNAL/page/issues/324)

## 0.7.0

- **Fix**: Invalid error message made for the CLI is used by all consumers [#312](https://github.com/JosephBARBIERDARNAL/page/issues/312)
- **Fix**: ToUnicode CMap ranges lack a cumulative expansion limit [#318](https://github.com/JosephBARBIERDARNAL/page/issues/318)
- **Fix**: Tagged-table spans can cause unbounded grid allocations [#317](https://github.com/JosephBARBIERDARNAL/page/issues/317)
- **Fix**: font stream decode fallback bypasses max_decoded_stream_size [#316](https://github.com/JosephBARBIERDARNAL/page/issues/316)
- **Fix**: encrypted PDFs can bypass the configured object-count limit [#315](https://github.com/JosephBARBIERDARNAL/page/issues/315)
- **Doc**: make a better landing page [#282](https://github.com/JosephBARBIERDARNAL/page/issues/282)
- **Doc**: add changelog page [#274](https://github.com/JosephBARBIERDARNAL/page/issues/274)
