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

`isPdfCompliantBytes()` is the fastest way to get a simple true/false compliance result for a profile. It expects the PDF as a `Uint8Array`:

```ts
import { isPdfCompliantBytes } from "page-validation-wasm";

const bytes = new Uint8Array(await pdfFile.arrayBuffer());
const isCompliant: boolean = await isPdfCompliantBytes(bytes);
```

If the profile isn't specified, it reads the PDF/A or PDF/UA profile declared in the document's XMP metadata. A missing, malformed, or unsupported profile declaration, or an input that cannot be read or parsed, throws `ValidationError`.

The first call initializes the WebAssembly module automatically. Call `initialize()` during application startup if you want to control initialization explicitly.

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
const report = await validatePdfBytes(bytes, ValidationProfile.PDF_A_1B);
```

The explicit-profile call does not require the document to contain a usable profile declaration. The declaration can still fail the selected profile's metadata rules. Use `isPdfCompliantBytes()` when you only need a boolean result.

## Failures

Each report contains a list of failures:

```ts
import { validatePdfBytes } from "page-validation-wasm";

const report = await validatePdfBytes(bytes);

for (const failure of report.failures) {
  console.log(`Rule: ${failure.ruleId}`);
  console.log(`Category: ${failure.category}`);
  console.log(`Message: ${failure.message}`);
}
```

`report.rules` contains the total, passed, and failed implemented rules. `report.checks.failed` counts every raw finding, so several objects failing the same rule contribute one failed rule and multiple failed checks.

Failure categories distinguish conformance problems from parser or operational errors:

```ts
import { FailureCategory } from "page-validation-wasm";

for (const failure of report.failures) {
  if (
    failure.category === FailureCategory.METADATA ||
    failure.category === FailureCategory.CONFORMANCE
  ) {
    // The PDF was parsed, but failed a validation rule.
  } else if (failure.category === FailureCategory.PARSER) {
    // The PDF could not be parsed correctly.
  } else if (failure.category === FailureCategory.OPERATIONAL) {
    // Validation failed because of I/O or another runtime issue.
  }
}
```

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

const report = await validatePdfBytes(bytes, undefined, limits);
```

You can also pass a partial options object instead of constructing `SafetyLimits`. `maxDecodedStreamSize` bounds one decoded stream and `maxTotalDecodedContentSize` bounds the combined decoded page, Form, appearance, Pattern, and Type3 content plus font streams retained by font inspection for one document. `maxFormInvocations` bounds Form XObject expansions across all pages and nested content in one document. `maxXrefRevisions` bounds the number of incremental-update revisions read from the cross-reference chain. `maxTableSpan` bounds the row or column span of an individual tagged-table cell. `maxTableGridRows`, `maxTableGridColumns`, and `maxTableGridCells` bound the derived table-grid dimensions and total cells. `maxUnicodeCmapMappings` bounds the total mappings expanded from one ToUnicode CMap.

For trusted files, pass `SafetyLimits.unlimited()` through the existing limits argument:

```ts
const report = await validatePdfBytes(bytes, undefined, SafetyLimits.unlimited());
```

The factory returns a fresh object with `Infinity` in every limit field. Wasm translates these values to native integer maxima. You can restore individual bounds by assigning finite values to its fields; partial options objects also accept positive `Infinity`. Validation may consume unrestricted memory and CPU; see the [safety limits guide](../guide/safety-limits.md).

## Use the exit code

For automated checks, a report can provide an appropriate process exit code:

```ts
import { validatePdfBytes } from "page-validation-wasm";

const report = await validatePdfBytes(bytes);
process.exitCode = report.exitCode();
```

The exit code is `0` for a compliant report, `2` for a noncompliant report, and `1` when the report contains an operational failure.

## Export the report

Validation reports can be exported as JSON:

```ts
import { validatePdfBytes } from "page-validation-wasm";

const report = await validatePdfBytes(bytes);
const json = report.toJson();
```
