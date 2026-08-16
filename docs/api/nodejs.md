---
title: "Node.js"
---

# Using page in Node.js

The Node.js package uses WebAssembly and accepts PDF data as a Node.js `Buffer` or `Uint8Array`. It initializes the WebAssembly module automatically.

## Installation

```sh
npm install page_validation_wasm
```

## Use the declared profile

`validatePdf` infers the profile from the PDF's XMP metadata:

```js
const { readFileSync } = require("node:fs");
const { validatePdf } = require("page_validation_wasm");

const bytes = readFileSync("document.pdf");
const report = validatePdf(bytes);

console.log(report.valid);
for (const failure of report.failures) {
  console.log(`[${failure.rule}] ${failure.message}`);
}
```

This function throws a JavaScript `Error` when the bytes cannot be parsed or when the profile declaration is missing, malformed, unsupported, or exceeds a safety limit.

## Select a profile explicitly

Use `validatePdfWithProfile` when the caller selects the validation profile:

```js
const { readFileSync } = require("node:fs");
const { validatePdfWithProfile } = require("page_validation_wasm");

const bytes = readFileSync("document.pdf");
const report = validatePdfWithProfile(bytes, "a-1b");
```

Explicit-profile validation does not require the PDF to declare a profile. Parser, operational, and conformance problems are represented in the returned report. An unknown profile string throws a JavaScript `Error`.

The accepted profile strings are `a-1b`, `a-1a`, `a-2b`, `a-2a`, `a-2u`,
`a-3b`, `a-3a`, `a-3u`, `a-4`, `a-4e`, `a-4f`, `ua-1`, and `ua-2`. PDF/A-1,
PDF/A-2, and PDF/A-3 profiles are supported by the preliminary validator.

## Validation report

Both functions return a validation report:

```json
{
  profile: "a-1b",
  valid: false,
  failures: [
    {
      rule: "PDFA1B-TRAILER-ID-001",
      message: "the applicable document trailer does not contain an ID entry",
    },
  ],
}
```

TypeScript declarations for the functions, reports, failures, errors, and profile strings are included in the package.
