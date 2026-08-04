---
title: "WebAssembly"
---

# Using page in WebAssembly

The WebAssembly package provides synchronous functions for validating PDF bytes held in a `Uint8Array`. 

## Use the declared profile

`validatePdf` infers the profile from the PDF's XMP metadata:

```js
import init, { validatePdf } from "./pkg/page_validation_wasm.js";

await init();

const response = await fetch("document.pdf");
const bytes = new Uint8Array(await response.arrayBuffer());
const report = validatePdf(bytes);

console.log(report.valid);
for (const failure of report.failures) {
  console.log(`[${failure.rule}] ${failure.message}`);
}
```

This function throws a JavaScript `Error` when the bytes cannot be parsed or when the profile declaration is missing, malformed, unsupported, or exceeds a safety limit. PDF conformance failures are returned in `report.failures`.

## Select a profile explicitly

Use `validatePdfWithProfile` when the caller selects the validation profile:

```js
import init, {validatePdfWithProfile} from "./pkg/page_validation_wasm.js";

await init();

const response = await fetch("document.pdf");
const bytes = new Uint8Array(await response.arrayBuffer());
const report = validatePdfWithProfile(bytes, "a-1b");
```

Explicit-profile validation does not require the PDF to declare a profile. Parser, operational, and conformance problems are represented in the returned report. An unknown profile string throws a JavaScript `Error`.

The accepted profile strings are `a-1b`, `a-1a`, `a-2b`, `a-2a`, `a-2u`, `a-3b`, `a-3a`, `a-3u`, `a-4`, `a-4e`, `a-4f`, `ua-1`, and `ua-2`. Profiles other than `a-1a` and `a-1b` currently return an unsupported-profile report.

## TypeScript

The generated declarations expose these signatures:

```ts
function validatePdf(data: Uint8Array): ValidationReport;

function validatePdfWithProfile(
  data: Uint8Array,
  profile: ValidationProfile,
): ValidationReport;
```

They also define `ValidationReport`, `ValidationFailure`, `ValidationError`, and the `ValidationProfile` string union.
