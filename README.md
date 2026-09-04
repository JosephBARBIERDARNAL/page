# page

A fast and lightweight PDF accessibility and compliance checker.

- [Documentation](https://josephbarbierdarnal.github.io/page/)
- [Installation](#installation)
- Usage: [CLI](https://josephbarbierdarnal.github.io/page/docs/api/cli), [Rust](https://josephbarbierdarnal.github.io/page/docs/api/rust), [Python](https://josephbarbierdarnal.github.io/page/docs/api/python) or [WebAssembly](https://josephbarbierdarnal.github.io/page/api/wasm/)
- [License](#license)

<br>

## Installation

`page` distribute pre-built binaries for macOS/Linux/Windows. If you're a Rust hacker, you can also install the latest dev version via Cargo.

### macOS/Linux

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/josephbarbierdarnal/page/releases/download/v0.4.0/page_cli-installer.sh | sh
```

### PowerShell

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/josephbarbierdarnal/page/releases/download/v0.4.0/page_cli-installer.ps1 | iex"
```

### Cargo

```sh
cargo install --git https://github.com/josephbarbierdarnal/page.git page_cli --bin page
```

<br>

## Quick start

### CLI

- Check accessibility (PDF/UA-1) compliance:

```console
$ page document.pdf --profile ua1
Result  : Non-conformant
Profile : PDF/UA-1
Time    : 0.052s
```

- Find reasons on non-conformance with `--format details`

```console
$ page document.pdf --profile ua1 --format details
Result  : Non-conformant
Profile : PDF/UA-1
Time    : 0.147s

[PDFUA1-ALT-TEXT-LANGUAGE-001] [.........]
[PDFUA1-CONTENT-TAGGING-001] [.........]
[PDFUA1-FIGURE-ALTERNATIVE-TEXT-001] [.........]
[PDFUA1-HEADING-NESTING-001] [.........]
[PDFUA1-ID-PART-001] [.........]
[PDFUA1-ID-SCHEMA-001] [.........]
[PDFUA1-LINK-CONTENTS-001] [.........]
[PDFUA1-METADATA-TITLE-001] [.........]
[PDFUA1-SPAN-ACTUAL-TEXT-LANGUAGE-001] [.........]
[PDFUA1-TABLE-COLUMN-ROWSPAN-001] [.........]
[PDFUA1-TABLE-HEADERS-SCOPE-001] [.........]
[PDFUA1-TABLE-ROW-COLUMNSPAN-001] [.........]
[PDFUA1-TAGGED-DOCUMENT-001] [.........]
[PDFUA1-TEXT-LANGUAGE-001] [.........]
```

> [!NOTE]
> `[ . . . . . . . . . ]` here are just placeholders of the actual messages

[Learn more about how to use the CLI.](https://josephbarbierdarnal.github.io/page/api/cli/)

### Rust

You can use the `page_validation` crate to integrate into any existing Rust workflow:

```rust
use std::path::Path;
use page_validation::{SafetyLimits, validate_pdf};

let doc = Path::new("document.pdf")
let report = validate_pdf(
   Path::new("file.pdf"),
   Some(ValidationProfile::PdfUA1),
   &SafetyLimits::default(),
)?;

if report.is_compliant {
    println!("The document passed all checks.");
} else {
    for failure in &report.failures {
        eprintln!(
            "[{}] {}",
            failure.rule_id,
            failure.message,
        );
    }
}
```

[Learn more about how to use the Rust crate.](https://josephbarbierdarnal.github.io/page/api/rust/)

### Python

You can use the `page-validation` Python package to integrate into any existing Python workflow:

```py
import page

report = page.validate_pdf("document.pdf")

if report.is_compliant:
    print("The document passed all implemented checks.")
else:
    for failure in report.failures:
        print(f"[{failure.rule_id}] {failure.message}")
```

[Learn more about how to use the Python package.](https://josephbarbierdarnal.github.io/page/api/python/)

### WebAssembly

Read a PDF as a `Uint8Array` and validate it in the browser:

```ts
import { ValidationProfile, validatePdfBytes } from "page-validation-wasm";

const bytes = new Uint8Array(await pdfFile.arrayBuffer());
const report = await validatePdfBytes(bytes, ValidationProfile.PDF_A_1B);

if (report.isCompliant) {
  console.log("The document passed all implemented checks.");
} else {
  for (const failure of report.failures) {
    console.log(`[${failure.ruleId}] ${failure.message}`);
  }
}
```

[Learn more about how to use the WebAssembly package.](https://josephbarbierdarnal.github.io/page/api/wasm/)

<br>

## License

`page`'s original source code and all other project-authored material are licensed under the MIT License. Everything not expressly identified as third-party or reference material in the accompanying [THIRD_PARTY_NOTICES.md](./crates/page_validation/THIRD_PARTY_NOTICES.md) is MIT. The `page_validation` crate additionally includes Adobe CMap Resources under BSD-3-Clause and adapted Mozilla PDF.js encoding tables under Apache-2.0. See the notice and license files for details.
