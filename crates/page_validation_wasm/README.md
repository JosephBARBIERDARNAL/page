# page_validation_wasm

WebAssembly bindings for the `page_validation` PDF/A validator.

The package exposes one synchronous function:

```ts
function validatePdf(data: Uint8Array): ValidationReport;
```

It infers the validation profile from the PDF's XMP metadata. PDF/A-1b is the
only profile currently implemented.

## Build

Build an ES module for browsers with:

```sh
wasm-pack build crates/page_validation_wasm --target web --release
```

Use a release build: the unoptimized validator currently exceeds
wasm-bindgen's per-function local-variable limit.

## Use

```js
import init, { validatePdf } from "./pkg/page_validation_wasm.js";

await init();

const response = await fetch("document.pdf");
const report = validatePdf(new Uint8Array(await response.arrayBuffer()));

console.log(report.valid);
for (const failure of report.failures) {
  console.log(`[${failure.rule}] ${failure.message}`);
}
```

`validatePdf` throws a JavaScript `Error` when the bytes cannot be parsed or
when the profile declaration is missing, malformed, unsupported, or exceeds a
safety limit. PDF/A conformance failures are returned in `report.failures`.

The generated TypeScript declarations define `ValidationReport`,
`ValidationFailure`, `ValidationError`, and `ValidationProfile`.

## License

The binding code is licensed under MIT. Its WebAssembly binary includes Adobe
CMap resources distributed under the BSD 3-Clause license; both license files
are included in the generated package.
