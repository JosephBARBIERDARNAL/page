---
title: "WebAssembly"
---

The [`page-validation-wasm`](https://www.npmjs.com/package/page-validation-wasm) npm package provides Wasm bindings for page.

## Installation

=== "npm"

      ```sh
      npm install page-validation-wasm
      ```

=== "pnpm"

      ```sh
      pnpm add page-validation-wasm
      ```

=== "yarn"

      ```sh
      yarn add page-validation-wasm
      ```

## Check compliance of a PDF

`isPdfCompliantBytes()` is the fastest way to get a simple true/false compliance result for a profile. It uses lazy validation: it stops once it finds a failing rule and returns the boolean directly. It expects the PDF as a `Uint8Array`:

```ts
import { isPdfCompliantBytes } from "page-validation-wasm";

const bytes = new Uint8Array(await pdfFile.arrayBuffer());
const isCompliant: boolean = await isPdfCompliantBytes(bytes);
```

Both validation functions take the bytes plus an optional `{ profile, limits }` options object. If the profile isn't specified, it reads the PDF/A or PDF/UA profile declared in the document's XMP metadata. Validation failures throw `PageError` with a `kind` such as `parser`, `safety_limit`, or `profile`, plus a `ruleId` such as `RESOURCE-LIMIT-001`.

The bundler loads and starts the WebAssembly module with the package, so no initialization call is required.

## Validate a PDF with details

If you need details about which rules failed, use `validatePdfBytes()`:

```ts
import { validatePdfBytes } from "page-validation-wasm";

const bytes = new Uint8Array(await pdfFile.arrayBuffer());
const report = await validatePdfBytes(bytes);

if (report.isCompliant) {
  console.log("The document passed all implemented rules.");
} else {
  console.log(`${report.rules.failed} rules failed`);
  console.log(`${report.checks.failed} checks failed`);
  for (const failure of report.failures) {
    console.log(`[${failure.ruleId}] ${failure.message}`);
  }
}
```

## Select a profile explicitly

Pass a profile to `validatePdfBytes()` when the caller, rather than the document, selects it:

```ts
import { ValidationProfile, validatePdfBytes } from "page-validation-wasm";

const bytes = new Uint8Array(await pdfFile.arrayBuffer());
const report = await validatePdfBytes(bytes, { profile: ValidationProfile.PDF_A_1B });
```

The explicit-profile call does not require the document to contain a usable profile declaration. The declaration can still fail the selected profile's metadata rules. Use `isPdfCompliantBytes()` when you only need a boolean result.

## Failures

Each report contains a list of failures:

```ts
import { validatePdfBytes } from "page-validation-wasm";

const report = await validatePdfBytes(bytes);

for (const failure of report.failures) {
  console.log(`Rule: ${failure.ruleId}`);
  console.log(`Message: ${failure.message}`);
}
```

`report.rules` contains the total, passed, and failed implemented rules. `report.checks.failed` counts every raw finding, so several objects failing the same rule contribute one failed rule and multiple failed checks.

Parser, profile, and safety-limit failures throw `PageError` with `kind` and `ruleId` properties and do not appear in a report.

## Safety limits

Safety limits protect the validator from excessively large or complex inputs. Defaults are sufficient for most cases:

```ts
import { SafetyLimits, validatePdfBytes } from "page-validation-wasm";

const limits = new SafetyLimits({
  maxInputSize: 256 * 1024 * 1024, // 256 MiB
  maxDecodedStreamSize: 32 * 1024 * 1024, // 32 MiB
  maxTotalDecodedContentSize: 256 * 1024 * 1024, // 256 MiB
  maxFormInvocations: 10_000, // Form XObject expansions per document
  maxObjectCount: 1_000_000, // 1,000,000 objects
  maxReferenceDepth: 256, // 256 levels
  maxXrefRevisions: 1_024, // 1,024 revisions
  maxTableSpan: 1_024, // rows or columns per cell
  maxTableGridRows: 1_024, // rows
  maxTableGridColumns: 1_024, // columns
  maxTableGridCells: 1_000_000, // cells
  maxUnicodeCmapMappings: 1_000_000, // mappings per ToUnicode CMap
});

const report = await validatePdfBytes(bytes, { limits });
```

You can also pass a partial options object instead of constructing `SafetyLimits`. `maxDecodedStreamSize` bounds one decoded stream and `maxTotalDecodedContentSize` bounds the combined decoded page, Form, appearance, Pattern, and Type3 content plus font streams retained by font inspection for one document. `maxFormInvocations` bounds Form XObject expansions across all pages and nested content in one document. `maxXrefRevisions` bounds the number of incremental-update revisions read from the cross-reference chain. `maxTableSpan` bounds the row or column span of an individual tagged-table cell. `maxTableGridRows`, `maxTableGridColumns`, and `maxTableGridCells` bound the derived table-grid dimensions and total cells. `maxUnicodeCmapMappings` bounds the total mappings expanded from one ToUnicode CMap.

For trusted files, pass `SafetyLimits.unlimited()` through the `limits` option:

```ts
const report = await validatePdfBytes(bytes, { limits: SafetyLimits.unlimited() });
```

The factory returns a fresh object with `Infinity` in every limit field. Wasm translates these values to native integer maxima. You can restore individual bounds by assigning finite values to its fields; partial options objects also accept positive `Infinity`. Validation may consume unrestricted memory and CPU; see the [safety limits guide](../guide/safety-limits.md).

## Check compliance

Use `isCompliant` to check whether the document passed all implemented checks:

```ts
import { validatePdfBytes } from "page-validation-wasm";

const report = await validatePdfBytes(bytes);
if (!report.isCompliant) {
  console.log("The document failed one or more implemented checks.");
}
```

Parser, profile, and safety-limit failures reject with `PageError` containing `kind` and `ruleId` before a report is returned.

## Export the report

Validation reports can be exported as JSON:

```ts
import { validatePdfBytes } from "page-validation-wasm";

const report = await validatePdfBytes(bytes);
const json = report.toJson();
```

`toJson()`, `toJSON()`, and `JSON.stringify(report)` use the same stable report schema as the CLI and Python bindings. Each JSON failure includes its `category` (`metadata` or `conformance`) and `object_id`, which contains `object_number` and `generation` when the finding is attributed to an indirect object and is `null` otherwise. The schema also includes the file when available, profile, validity, rule and check counts, and optional parser or operational error details. Wasm terminal errors reject with `PageError` carrying `kind` and `ruleId` before a report is returned.
