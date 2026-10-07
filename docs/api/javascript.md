---
title: "JavaScript"
---

The [`page-validation-wasm`](https://www.npmjs.com/package/page-validation-wasm) npm package provides a JavaScript API for page, powered by WebAssembly and designed to run in the browser (see [this issue](https://github.com/JosephBARBIERDARNAL/page/issues/284) for Node.js support).

## Installation

=== "npm"

      ```sh
      npm install page-validation-wasm
      ```

=== "bun"

      ```sh
      bun add page-validation-wasm
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

`isPdfCompliantBytes()` is the fastest way to get a simple true/false compliance result for a profile. It uses [lazy validation](../guide/lazy-validation.md): it stops once it finds a failing rule and returns the boolean directly:

```ts
import { isPdfCompliantBytes, ValidationProfile } from "page-validation-wasm";

const bytes = new Uint8Array(await pdfFile.arrayBuffer());

await isPdfCompliantBytes(bytes, { profile: ValidationProfile.PDF_UA_1 });
```

Both validation functions take the PDF bytes plus an optional `{ profile, limits }` options object. The WebAssembly module loads and starts with the package, so no initialization call is required.

## Validate a PDF with details

If you need details about **which rules failed**, use `validatePdfBytes()` instead:

```ts
import { ValidationProfile, validatePdfBytes } from "page-validation-wasm";

const bytes = new Uint8Array(await pdfFile.arrayBuffer());
const report = await validatePdfBytes(bytes, { profile: ValidationProfile.PDF_UA_1 });

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

## Profile selection

If `profile` isn't specified, it reads the PDF/A or PDF/UA profile declared in the document's XMP metadata. Since this isn't the case for all PDFs, a document that doesn't declare it will reject with `PageError`:

```ts
import { validatePdfBytes } from "page-validation-wasm";

const bytes = new Uint8Array(await pdfFile.arrayBuffer());
const report = await validatePdfBytes(bytes);
```

The profile is a `ValidationProfile`:

```ts
import { ValidationProfile, validatePdfBytes } from "page-validation-wasm";

const bytes = new Uint8Array(await pdfFile.arrayBuffer());

const report = await validatePdfBytes(bytes, { profile: ValidationProfile.PDF_UA_1 });
```

You can find all available profiles with:

```ts
import { ValidationProfile } from "page-validation-wasm";

Object.values(ValidationProfile);
```

```ts
["1a", "1b", "2a", "2b", "2u", "3a", "3b", "3u", "ua1"];
```

## Failures

Each report contains a list of failures:

```ts
import { ValidationProfile, validatePdfBytes } from "page-validation-wasm";

const report = await validatePdfBytes(bytes, { profile: ValidationProfile.PDF_UA_1 });

for (const failure of report.failures) {
  console.log(`Rule: ${failure.ruleId}`);
  console.log(`Category: ${failure.category}`);
  console.log(`Message: ${failure.message}`);
}
```

`report.rules` contains the total, passed, and failed implemented rules. `report.checks.failed` counts every raw finding, so several objects failing the same rule contribute one failed rule and multiple failed checks.

Failure categories describe findings from rules that ran on a parsed document:

```ts
import { validatePdfBytes } from "page-validation-wasm";

const report = await validatePdfBytes(bytes, { profile: ValidationProfile.PDF_UA_1 });

for (const failure of report.failures) {
  if (failure.category === "metadata") {
    console.log("metadata finding", failure.ruleId);
  } else if (failure.category === "conformance") {
    console.log("conformance finding", failure.ruleId);
  }
}
```

Parser, profile, and safety-limit failures reject with `PageError` and do not appear in a report. `PageError.kind` identifies the error as `input_io`, `parser`, `profile`, `safety_limit`, or `unknown`; `PageError.ruleId` contains the associated rule identifier when available.

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

For **trusted files**, pass `SafetyLimits.unlimited()` through the `limits` option:

```ts
import { SafetyLimits, validatePdfBytes } from "page-validation-wasm";

const report = await validatePdfBytes(bytes, { limits: SafetyLimits.unlimited() });
```

The factory returns a fresh object with `Infinity` in every limit field. The WebAssembly layer translates these values to native integer maxima. You can restore individual bounds by assigning finite values to its fields; partial options objects also accept positive `Infinity`. Validation may consume unrestricted memory and CPU; see the [safety limits guide](../guide/safety-limits.md) for details about each limit.

## Export the report

Validation reports can be exported as JSON:

```ts
import { validatePdfBytes } from "page-validation-wasm";

const report = await validatePdfBytes(bytes);
const json = report.toJson();
```

Use `report.toJSON()` or `JSON.stringify(report)` to get the same report structure as a JavaScript object:

```ts
const reportData = report.toJSON();
console.log(reportData.is_compliant);
console.log(reportData.failures);
```

Each JSON failure includes `category` (`metadata` or `conformance`) and `object_id`. The object ID contains `object_number` and `generation` when the finding is attributed to an indirect object, and is `null` otherwise.
