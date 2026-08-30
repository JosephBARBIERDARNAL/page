# Plan: Heuristic Inspection Planning for Faster Validation

## Objective

Add internal, behavior-preserving heuristics that avoid expensive validation work when the document and selected profile prove that a rule family cannot apply, while keeping the same profiles, implemented rules, failure messages and locations, report shape, ordering, profile-selection behavior, safety limits, parser behavior, operational errors, and CLI exit statuses.

The first target is wasted inspection work such as detailed font analysis for a document whose reachable content contains no shown text, but the design should support other resource families such as images, annotations, forms, actions, colour spaces, and structure-related content without introducing a generic rule engine that is harder to verify than the existing code.

This is an implementation plan, not a proposal to weaken validation. A heuristic may skip work only after a complete and conservative applicability fact proves that the skipped work cannot affect the externally observable result; otherwise the current inspection path remains the fallback.

## Scope and non-goals

- Keep the public APIs in `page_validation` and `page_cli` unchanged unless implementation reveals an unavoidable compatibility issue that is explicitly approved.
- Keep all currently implemented profile rules and the current `ValidationProfile::implemented_check_count` semantics unchanged; skipping an inapplicable rule must not reduce the reported profile check total.
- Do not infer applicability from file size, file name, producer, extension, or other heuristics that can be wrong for valid PDFs.
- Do not change veraPDF rule interpretation, reachability semantics, failure wording, deterministic ordering, fixture bytes, or documentation outside this requested `plan.md` file.
- Do not optimize by silently ignoring malformed or over-limit objects; parser and safety behavior must either remain covered by a mandatory scan or cause the heuristic to fall back to the existing path.
- Do not introduce `#[allow(...)]` attributes or relax lint settings.

## Current architecture and confirmed cost model

### Public request paths

- `validate_pdf` in `crates/page_validation/src/validation.rs` reads the file under `SafetyLimits`, then delegates to byte validation in exhaustive mode.
- `validate_pdf_bytes` rejects explicitly unsupported profiles, calls `PdfDocument::from_bytes_with_inspections`, infers the profile only after that inspection construction when the caller passed `None`, and then calls `validate_document` in exhaustive mode.
- `is_pdf_compliant` and `is_pdf_compliant_bytes` are the compact fast path; they prepare and normalize the document, select the profile before inspections, run a preflight check, build the same inspection summaries, and call `validate_document` in `FirstFailure` mode.
- Explicit profiles are rejected before parsing when unimplemented; automatic profile selection uses the PDF/A or PDF/UA XMP identification declarations from the normalized document.

### Preparation and normalization

- `PdfDocument::prepare_for_validation` in `crates/page_validation/src/model.rs` performs strict `lopdf` loading with a decoded-stream bound, encryption detection, object-count enforcement, catalog resolution, and one bounded page-tree collection.
- `PdfDocument::normalize` extracts trailer and document metadata, catalog metadata, XMP, output intents, page count, an informational whole-document font summary, and object count.
- The normalized `PdfDocument::fonts` value is intentionally an informational count of every `/Type /Font` object and must not be replaced with the content-reached font population used by conformance rules.
- The normalization font summary currently walks every parsed object and resolves font descriptors to count embedded font keys; it is part of the serialized report and therefore needs independent compatibility coverage if optimized.

### Shared inspection construction

- `ValidationPreparation::into_inspections_with_syntax` currently creates an `InspectionSummary` containing header, content, font, ICC, XObject, graphics, action, form, annotation, document-feature, object-limit, stream-safety, and Unicode-name summaries.
- The current order is document-feature inspection, one shared bounded content execution, ICC inspection, XObject inspection, graphics inspection, actions, forms, annotations, stream safety, Unicode names, and font embedding.
- The shared content execution and its `ContentCache` are an important existing optimization: decoded page, Form XObject, appearance, Pattern, and Type3 streams are reused, while resource context and graphics state are re-applied for each invocation.
- The shared content summary intentionally establishes the resource population used by colour, XObject, graphics, and font predicates, so a new planner must not add separate full page/content traversals for each rule family.

### Profile-specific evaluation

- `validate_document` in `crates/page_validation/src/validation.rs` contains an early PDF/UA-1 branch and a PDF/A branch; each aggregates findings from the summaries according to the selected profile.
- PDF/UA-1 consumes document features, content findings, actions, forms, annotations, reachable Form XObject findings, and selected font findings, then returns before the generic PDF/A evaluator.
- PDF/A profiles consume metadata and identification checks, optional tagged-structure checks for conformance level A, Unicode-related checks for A and U profiles, output intents, ICC findings, XObjects, graphics, annotations, actions, forms, document features, stream safety, object limits, and profile-specific font findings.
- Profile predicates already exist for PDF/A part, conformance level, tagged-structure requirements, Unicode mapping, optional content, transparency, embedded files, and cross-reference stream support; reuse these methods instead of duplicating profile logic.
- The summary CLI uses `ValidationMode::FirstFailure`, but this currently stops only the final failure aggregation; it does not avoid the preceding inspection pass.

### Font applicability already present

- The content executor records a `FontUse` only when a `Tf`-selected resource is followed by a text-show operator with non-empty shown bytes; this includes text reached through the bounded content graph and retains rendering mode information.
- `font_embedding::inspect` consumes `execution.fonts` for detailed font dictionary, embedding, glyph, width, encoding, Unicode, CMap, and subset checks, so an empty content-reached font population is already a strong applicability fact for most font work.
- Font rendering mode 3 must not be treated as “no font use”; PDF/UA-1 has checks that intentionally retain invisible text-showing operators, and the existing scanner preserves those uses.
- `font_embedding::inspect` also runs `inspect_all_embedded_cmap_cids`, which scans all parsed stream objects carrying `/CMapName`; this is a separate whole-document population and cannot be skipped merely because `execution.fonts` is empty without differential proof.
- The existing `canonical-pdfa-1a-unused-invalid-font.pdf` fixture and related maintenance generator are specifically relevant evidence that an invalid unused font is not automatically a font-rule failure, but they do not by themselves prove that every CMap or operational scan may be skipped.

### Likely work and repeated scans

- `syntax::inspect` walks xref-addressable source objects and raw stream locations to enforce syntax and object/string limits; this is broad safety coverage and should be treated as mandatory until its rule-specific portions are explicitly separated.
- `stream_safety::inspect` walks all objects and streams for raw stream measurements, external stream entries, filters, signatures, and nested stream ranges; it can return resource errors and has whole-document semantics, so it is a high-risk candidate for whole-inspector skipping.
- `document_features::inspect` combines cheap catalog checks with structure-tree traversal and whole-object scans for file specifications, associated files, permissions, and embedded files; it also recursively validates embedded PDF/A files and must be split carefully before any profile or feature gating.
- `content_support::execute_content` decodes and tokenizes reachable content streams and tracks fonts, colour spaces, XObjects, ExtGStates, transparency, marked content, language, syntax limits, and content object limits; it is central shared work rather than an inspector to duplicate.
- `icc_based::inspect` examines selected content colour spaces but also scans all objects for DeviceN-related findings and checks separation consistency, so its selected-colour and whole-document portions need separate applicability decisions.
- `xobject::inspect` consumes only content-reached XObject uses and is a relatively clear candidate for an early default return when the complete content summary has no XObjects, subject to preserving any error-producing work currently performed elsewhere.
- `graphics::inspect` consumes reached ExtGState/XObject uses and page transparency state but also scans all objects for halftones and ExtGState dictionaries, so only sub-scans can be gated from existing facts.
- `actions::inspect`, `forms::inspect`, and `annotations::inspect` each walk pages or catalog structures independently even when their resulting vectors are empty; a shared presence/reachability index could avoid unnecessary work, but widget, appearance, outline, XFA, and additional-action populations must remain distinct.
- `unicode_names::inspect` walks inherited page resources and is only consumed by the PDF/A-2 and PDF/A-3 evaluator; skipping it for other implemented profiles is attractive, but its possible reference-depth errors must be covered by the operational-compatibility decision.

## Required invariants

The implementation is acceptable only if all of the following remain true.

- For every currently supported profile and fixture, the complete ordered failure list, failure category, rule ID, message, object location, compliance boolean, report counts, normalized document fields, and source path remain unchanged.
- A compact result remains first-failure in the same validation order, while detailed output remains exhaustive in the same deterministic order.
- Explicit profile selection continues to override XMP declarations; automatic selection continues to reject absent, ambiguous, invalid, or unsupported declarations with the same error classes and precedence.
- Input-size, decoded-stream, total decoded-content, object-count, and reference-depth limits remain enforced wherever the current API enforces them.
- Encrypted and content-unavailable PDFs continue to follow the existing special path and produce the same report or error behavior.
- A malformed or over-limit object that currently causes an operational failure cannot become invisible solely because a conformance rule does not use its findings.
- The informational `document.fonts` summary and all JSON/text output contracts remain unchanged.
- The existing profile coverage accounting continues to describe implemented rules, not the number of positive findings or the number of non-empty per-document populations.

## Proposed design

### Use a small internal inspection plan, not a generic dynamic rule registry

Introduce an internal plan type near the existing preparation/inspection boundary, with names chosen to match the code after implementation, such as `InspectionPlan` or `InspectionNeeds`.

The plan should carry two kinds of decisions.

- Profile needs: which inspector families and profile variants can contribute to the selected profile’s report.
- Document applicability: which expensive sub-inspections are proven relevant after the complete shared discovery facts are available.

Represent families with explicit fields or a compact internal bitset rather than attaching opaque metadata to every rule. The current evaluator is organized around families and summary vectors, and a family-level plan is easier to review against veraPDF and the existing rule mapping.

The plan must distinguish `required`, `applicable`, and `unknown` where needed. `false` is safe to skip only when it means “complete discovery proved absence”; `unknown` must execute the existing conservative path.

### Preserve a mandatory safety/report baseline

Separate work into a mandatory baseline and optional conformance inspections.

- Mandatory baseline remains strict PDF parsing, input and object limits, bounded page-tree collection, normalized metadata/XMP/profile information, and any syntax/raw-stream checks required to preserve current operational behavior.
- Optional conformance inspections produce the existing summary vectors and can be gated only when their applicability facts are complete and their skipped code cannot be the source of a current operational error.
- If an inspector currently mixes conformance findings with operations that can return `PdfError`, split those responsibilities or retain the error-producing portion in the baseline before gating the conformance portion.
- Do not represent a skipped summary as a new public “not checked” result; internally use the same empty summary only when the skipped work is proven to have no observable findings, and keep profile check totals unchanged.

### Reuse the existing shared discovery facts

Do not add one discovery walk per family. Extend `ContentExecutionSummary` and, where necessary, `DocumentFeatureSummary` with small booleans or sets that are already derivable during their existing walks.

Useful facts include whether any shown text reached a font, whether any content-reached XObject exists, whether ordinary images or masks exist, whether ExtGStates or transparency were used, whether colour spaces were selected, whether annotations or widgets exist on any page, whether a catalog AcroForm or action entry exists, and whether structure or associated-file data exists.

Facts must be recorded after the complete bounded traversal, including nested Form XObjects, annotation appearances, Patterns, and Type3 execution paths that the current content executor already models.

Do not use a partially populated vector during traversal to decide that a later nested resource is absent. Do not collapse resource presence, resource reachability, painted visibility, and text visibility into one boolean when veraPDF distinguishes them.

### Select the profile at the earliest safe point

For an explicit profile, the planner can know the profile before any expensive inspection.

For automatic selection, normalization already parses the XMP identification data in `prepare_for_validation`, so the valid inferred profile can be obtained before optional inspection construction. However, `validate_pdf_bytes` currently performs inspections before returning a missing or invalid inferred-profile error, while `is_pdf_compliant_bytes` selects first; changing this ordering can change observable error precedence.

Implement this in two steps: first record baseline behavior for invalid or missing profile declarations and retain the current fallback path for cases where profile selection cannot succeed; only use a profile-specific plan after a valid profile is known. Revisit profile-first construction after operational precedence tests exist.

Keep `reject_unimplemented_profile` before expensive work for explicit profiles, as it is already part of the current contract.

### Profile applicability matrix

Maintain the following matrix as an implementation checklist and update it only with evidence from the evaluator and veraPDF differential tests.

| Family                                 | Profile/use decision                                        | Document applicability fact                                          | Safe initial optimization                                                                                                                        |
| -------------------------------------- | ----------------------------------------------------------- | -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| PDF/UA-only content findings           | Needed only by PDF/UA-1 currently implemented               | PDF/UA content inspection flag and complete content traversal        | Avoid PDF/UA-only bookkeeping for PDF/A while retaining shared parsing needed by other rules                                                     |
| Tagged structure and language          | Needed by PDF/A conformance A and PDF/UA-1                  | Profile requirement plus structure/content facts                     | Do not run A-only rule summaries for B profiles; preserve required catalog absence checks                                                        |
| Unicode mapping/value checks           | Needed by PDF/A A/U and selected PDF/UA-1 font checks       | Profile methods plus content-reached font/text population            | Skip irrelevant profile variants, never skip rendering-mode-3 or nested text uses                                                                |
| Font dictionary/embedding/glyph checks | Needed by PDF/A and selected PDF/UA profiles                | Complete `ContentExecutionSummary::fonts` population                 | Skip usage-bound detailed font scanners when no font was reached by shown text; retain whole-document CMap and safety portions until proven safe |
| XObjects/images                        | Needed by applicable PDF/A/PDF/UA rules                     | Complete `content.xobjects` with role information                    | Early-return `xobject::inspect` only when it has no globally applicable work and no error-producing work outside reached XObjects                |
| Graphics/ExtGState/transparency        | Needed by PDF/A rules and selected PDF/UA content semantics | Reached ExtGState/XObject/transparency facts plus page groups        | Gate reached-resource sub-scans; keep whole-object halftone scans unless their rule population is proven reachable/profile-specific              |
| Selected colour spaces/ICC             | Needed by PDF/A rules                                       | Selected colour-space and default-colour facts                       | Skip selected-space parsing when no selected space exists; keep DeviceN whole-object checks unless independently proven unnecessary              |
| Annotations/widgets                    | Needed by PDF/A and PDF/UA rules                            | Complete page annotation/widget presence and appearance reachability | Avoid repeated page walks for empty pages; preserve widget, appearance, and annotation-specific populations                                      |
| Forms/XFA                              | Needed by PDF/A and PDF/UA rules                            | Catalog AcroForm/XFA and page widget facts                           | Skip form field traversal only when no AcroForm and no widgets are present; retain catalog-level checks when AcroForm exists                     |
| Actions/outlines                       | Needed by PDF/A and PDF/UA rules                            | Catalog/page/annotation/field/outline action-key presence            | Avoid recursive action traversal when no candidate action entry exists; preserve all direct forbidden-key checks                                 |
| Embedded files/associated files        | Needed mainly by PDF/A-2/3 and profile-specific rules       | Catalog/page/annotation/document-object association facts            | Profile-gate PDF/A-2/3-only embedded-file validation, but preserve whole-object embedded-file checks consumed by the selected profile            |
| Unicode resource names                 | Consumed by PDF/A-2/3 only                                  | Selected profile and complete page-resource population               | Skip for profiles that do not consume the summary only after operational behavior is preserved                                                   |
| Syntax/stream/object limits            | Cross-cutting safety and conformance                        | Raw object/stream coverage                                           | Keep mandatory until rule-specific and error-specific portions are separated                                                                     |

The matrix is deliberately conservative. A later phase may add a skip only after a focused rule applicability test demonstrates that the target population is exactly the one veraPDF evaluates.

## Implementation phases

### Phase 0: Establish a reproducible baseline

- Capture release-mode timings for the five existing `bench/document*.pdf` files using the current `bench/benchmark.rs` workflow, keeping the generated benchmark documentation out of the change unless separately requested.
- Measure both CLI modes: default compact output and `--format details`, because first-failure aggregation currently does not eliminate the inspection pass and the optimization may affect both paths differently.
- Add temporary or internal-only timing instrumentation around preparation, syntax inspection, content execution, and each inspector, or use a release profiler such as `cargo flamegraph` or the platform’s sampling profiler; remove temporary instrumentation unless it becomes a deliberate stable diagnostic feature.
- Record CPU time, wall time, peak memory or allocation volume when available, decoded content volume, object count, page count, and the sizes of the major discovered populations.
- Establish exact baselines for serialized reports, failure ordering, error variants, CLI exit codes, and profile-selection behavior across the existing atomic, canonical, corpus, and differential fixtures.
- Build a small workload matrix containing empty/no-text PDFs, text-heavy PDFs, image-heavy PDFs, font-heavy PDFs, annotation/form PDFs, embedded-file PDFs, PDF/A-1/2/3 profiles, PDF/UA-1, and malformed or limit-bound inputs.

### Phase 1: Add a testable internal applicability model

- Add a private plan/discovery representation at the model-to-inspection boundary without changing public types or serialized output.
- Add profile methods for inspection needs only when existing `ValidationProfile` predicates do not already express the decision; reuse `pdfa_part`, `pdfa_conformance`, `requires_tagged_structure`, `requires_unicode_mapping`, and permission methods.
- Derive initial facts from the existing `ContentExecutionSummary` and `DocumentFeatureSummary`; do not introduce a second page-tree or content-stream walk.
- Add unit tests for plan construction for every implemented profile, including PDF/A-1a, PDF/A-1b, PDF/A-2a/b/u, PDF/A-3a/b/u, and PDF/UA-1.
- Add tests asserting that incomplete or ambiguous discovery selects the conservative “run” path rather than skipping an inspector.
- Keep the old all-inspections path available behind an internal baseline function or test-only comparison hook until the optimized path is validated.

### Phase 2: Make the inspection boundary plan-aware without changing skip decisions

- Refactor `ValidationPreparation::into_inspections_with_syntax` to accept an internal plan or needs object while initially requesting all current inspection families.
- Preserve the current inspection order and shared `ContentCache` behavior during this refactor.
- Ensure defaulted summaries have the same effect on `validate_document` as the current summaries and that `ValidationReport` still uses the static implemented-check count.
- Keep explicit and inferred profile error precedence covered by tests before moving profile selection or changing when inspections are constructed.
- Run the complete offline tests and compare the baseline and plan-aware reports byte-for-byte where the existing tests expose serialized output.

### Phase 3: Apply the first low-risk heuristics

- Gate usage-bound font detail work on the complete content-reached font population, starting with the no-shown-text case requested by the user.
- Preserve `PdfDocument::fonts` normalization because it is an informational report field, even when no font is used by content.
- Split `inspect_all_embedded_cmap_cids` from usage-bound font scans and verify its exact population with veraPDF before considering a skip.
- Gate `xobject::inspect` on its actual content-reached population only if the function has no independent whole-document work or operational error behavior; otherwise gate only its expensive image/form subroutines.
- Gate profile-irrelevant Unicode-name and PDF/A-variant work only after checking every caller in `validate_document` and preserving errors from any shared resolver walks.
- Avoid whole-family skips for graphics, ICC, document features, actions, forms, annotations, and stream safety in this phase unless a function-level audit proves the family is entirely reachability-bound and error-neutral.
- After each heuristic, compare the optimized path with the retained baseline on all fixtures and measure whether the expected expensive operation actually disappeared.

### Phase 4: Split mixed inspectors into mandatory and optional work

- For each candidate whole-family optimization, identify which operations can produce a conformance finding, which operations populate report metadata, and which operations can return `PdfError` or enforce a safety bound.
- Extract mandatory safety/error checks from optional profile-specific summaries where they are currently mixed, keeping the same error type, limit value, and failure precedence.
- Add explicit summary constructors for “not needed” only internally; avoid conflating “not run because proven inapplicable” with parser failure or unavailable encrypted content.
- Use the planner to avoid constructing profile variants that the selected profile never consumes, particularly for PDF/UA-1 versus PDF/A-only summaries and PDF/A-1 versus PDF/A-2/3 variants.
- Recheck the planner whenever a new validation profile or rule family is added so new features default to the conservative all-work path until applicability is mapped.

### Phase 5: Optimize repeated traversal and allocation only where measurement justifies it

- Reuse the existing collected page list and content cache for all participating inspectors; do not reintroduce independent `document.get_pages()` calls.
- Share presence/reachability facts for annotations, widgets, forms, actions, and XObjects so empty-family checks do not repeatedly resolve the same page resources.
- Inspect allocation profiles for temporary cloned failure vectors and collect-then-filter paths in `validate_document`; change them only when profiling shows material cost and preserve deterministic ordering.
- Keep `BTreeMap`/`BTreeSet` ordering where it determines stable report output; do not replace collections solely for style.
- Do not add concurrency or unsafe parsing as part of this optimization; the validator is CPU-bound and the first-order opportunity is avoiding work and duplicate walks.

## Font-specific implementation sequence

The font case should be the first end-to-end heuristic because the repository already exposes the correct content-reached population.

- Confirm with existing and new fixtures that a PDF containing only unused font dictionaries does not receive ordinary font conformance failures, while the normalized informational font count remains present.
- Add an internal distinction between `font_usage_present` and `font_detail_required`; `font_usage_present` must include invisible rendering mode 3 text and text reached in nested Form XObjects, annotation appearances, Patterns, and Type3 paths supported by the current executor.
- When `execution.fonts` is empty, avoid invoking usage-bound scanner loops that can only inspect entries in the empty population.
- Keep the global embedded-CMap CID scan unchanged in the first patch and measure it separately; only gate it after a veraPDF-backed applicability matrix covers unused CMaps, reached CMaps, malformed CMaps, and over-limit CMaps.
- Preserve `font_embedding::inspect` errors for malformed or over-limit data unless the moved code is proven not to be part of the current error contract; a missing font conformance finding and a missing operational error are different regressions.
- Verify positive cases with visible text, invisible text, simple fonts, Type 0 fonts, Type 3 fonts, nested forms, repeated aliases, missing glyphs, missing Unicode mappings, and invalid embedded programs.

## Verification strategy

### Local behavior tests

- Add focused internal tests for plan decisions and summary defaults beside the owning modules, following the existing test style.
- Add integration coverage for no-text PDFs with unused invalid fonts, empty content streams, fonts selected but never used, and fonts used only in nested or invisible text paths.
- Add analogous absence/presence tests for images, XObjects, annotations, widgets, AcroForms, actions, colour spaces, DeviceN objects, structure trees, and embedded files.
- Preserve and extend tests that assert deterministic failure ordering, attached object IDs, report counts, serialized document summaries, and operational `ValidationError` variants.
- Exercise both exhaustive `validate_pdf_bytes` and first-failure `is_pdf_compliant_bytes`; do not assume an optimization affecting one path affects the other.

### veraPDF differential checks

- Run the pinned veraPDF 1.30.x executable through the repository’s `DifferentialRunner` and `just verapdf` workflow for every heuristic-related fixture.
- For each proposed skip, create the smallest possible positive, negative, absent, unreachable, nested, invisible, and malformed cases and compare the local failure set and applicability with veraPDF.
- Use the existing mutation manifest, atomic rule tests, canonical fixtures, and profile re-identification helpers instead of inventing a parallel expected-result format.
- Treat any unexpected veraPDF/local difference as a coverage or applicability problem requiring a minimal reprex before changing the heuristic.
- Run `just verapdf-all` and the corpus gate for the implemented profile set before considering a whole-inspector skip complete.

### Operational compatibility checks

- Test malformed syntax, broken references, cyclic and over-deep references, oversized decoded streams, excessive total decoded content, too many objects, encrypted content, malformed XMP, and malformed embedded files in both reachable and apparently unreachable locations.
- Compare whether the old and optimized paths return the same `ValidationError` variant and limit value, not merely whether both commands exit nonzero.
- Verify that early profile-selection changes do not alter missing/invalid profile error precedence for `validate_pdf_bytes`.
- Verify that skipped inspection summaries cannot suppress a failure that is required by a profile’s rule mapping or a required metadata/document-level absence rule.

## Benchmark and profiling matrix

Use release builds with the same compiler, target, feature flags, CPU, limits, warmup, and input order for baseline and optimized measurements.

| Workload                            | What it isolates                                    | Measurements                                                      |
| ----------------------------------- | --------------------------------------------------- | ----------------------------------------------------------------- |
| Small empty/no-text PDF             | Font and content-family no-op overhead              | End-to-end time, content decode time, font scan time, allocations |
| Text-heavy PDF with many used fonts | Reachable font parsing and glyph checks             | Font scanner CPU, decoded font bytes, allocations, total time     |
| PDF with many unused font objects   | Whole-object font summary versus usage-bound checks | Object walk time, font detail time, report equality               |
| Image-heavy PDF                     | XObject, JPX, colour, graphics work                 | XObject/ICC/graphics time, decoded image bytes, peak memory       |
| Annotation/form-heavy PDF           | Repeated page, widget, appearance, action walks     | Page traversal count, appearance decode time, allocations         |
| Structure-heavy PDF                 | Document feature and PDF/UA structure work          | Structure traversal time, PDF/A versus PDF/UA comparison          |
| Embedded-file PDF                   | Recursive embedded PDF/A validation                 | Nested validation time, recursion depth, report/error equality    |
| Large existing benchmark PDFs       | Realistic end-to-end regression                     | Median/p95 wall time and peak memory for compact/details modes    |
| Malformed and limit-bound PDFs      | Conservative fallback and operational behavior      | Error variant, failure precedence, time, no hidden work removal   |

Use the existing `bench/benchmark.rs` for broad CLI timing and add a focused internal or temporary harness for per-inspector timings. If a stable benchmark is added to the repository, keep it separate from production behavior and avoid regenerating checked-in documentation as an incidental side effect.

The plan must report measured gains by workload rather than claiming that an empty vector or fewer function calls are faster. A heuristic that adds a second traversal or large planner allocation is a regression unless its measured savings exceed its cost.

## Expected file touch points

- `crates/page_validation/src/model.rs`: preparation/inspection boundary, plan-aware inspection construction, and possibly shared discovery facts.
- `crates/page_validation/src/validation.rs`: profile-selection staging, profile needs, conservative fallback, and unchanged report aggregation.
- `crates/page_validation/src/content_support.rs`: additional complete discovery flags only when they can be populated during the existing shared execution.
- `crates/page_validation/src/font_embedding.rs`: split or gate usage-bound font scans while retaining global CMap and operational behavior until proven safe.
- `crates/page_validation/src/xobject.rs`, `graphics.rs`, `icc_based.rs`, `actions.rs`, `forms.rs`, `annotations.rs`, `document_features.rs`, `unicode_names.rs`, and `stream_safety.rs`: only the family-specific gates or mandatory/optional splits justified by profiling and applicability tests.
- `crates/page_validation/src/report.rs`: only if internal skipped-summary semantics require a private change; public report fields and counts must remain compatible.
- `crates/page_validation/tests/common/mod.rs`: reusable minimal fixtures, report comparison helpers, and profile matrix helpers.
- New focused integration tests under `crates/page_validation/tests/`, with existing fixture integrity and differential manifests updated only when new binary fixtures are intentionally added.
- `bench/benchmark.rs`: only if stable measurement coverage is needed; do not modify `docs/benchmark.md` unless explicitly requested.

## Acceptance criteria

- Every existing offline test passes, including all workspace tests with all features.
- `just fmt && just lint` is green before submission of implementation changes.
- Existing atomic, canonical, differential, and corpus validation results remain unchanged for every implemented profile.
- The exact report and error compatibility suite passes for reachable, unreachable, nested, invisible, malformed, encrypted, and limit-bound cases.
- The no-text/unused-font workload demonstrates that usage-bound font analysis is avoided without changing the informational font summary or any validation result.
- Each additional heuristic has a focused applicability test, a veraPDF differential case where relevant, and a conservative fallback for unknown reachability or parser state.
- Release-mode measurements show a reproducible improvement on at least one representative workload with no material regression on the rest of the benchmark matrix.
- The final implementation contains no undocumented rule skip, no weakened safety bound, no suppressed lint, and no new public “not checked” semantics.

## Recommended implementation order

1. Baseline timings, reports, errors, and applicability fixtures.
2. Internal plan type and plan-aware inspection boundary with all decisions set to “run.”
3. Font usage heuristic using the existing complete `ContentExecutionSummary::fonts` population, while retaining global CMap and safety scans.
4. Profile-specific gating for clearly unused summary variants, beginning with families already excluded by the evaluator for a profile.
5. Low-risk early returns for purely reachability-bound inspectors such as content-reached XObjects after error-surface review.
6. Mandatory/optional splits for mixed whole-document inspectors, only where operational compatibility can be demonstrated.
7. Shared presence/reachability facts to remove repeated page/catalog walks.
8. Re-profile, run all local and veraPDF-backed gates, and retain only changes with measured benefit.

Every material phase should end with the same report/error comparison and a release-mode measurement checkpoint before the next phase begins.
