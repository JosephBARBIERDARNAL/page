---
hide:
  - navigation
  - toc
---

<div class="home-page" markdown>

<div class="home-hero" markdown>

<div class="home-hero__content" markdown>

<div class="home-eyebrow">PDF accessibility and validation</div>

# Fast PDF accessibility & compliance validation

Check PDF documents against accessibility and compliance requirements with a Rust-based engine that runs as a CLI, Rust crate, Python package, or WebAssembly module.

[Get started](installation.md){ .md-button .md-button--primary }
[Try the demo](demo.md){ .md-button }

</div>

<div class="home-hero__preview" aria-label="Example successful page validation">
<div class="home-preview__bar"><span class="home-preview__dots"><i></i><i></i><i></i></span></div>
<div class="home-preview__body">
<div class="home-preview__command">$ page document.pdf --profile ua1</div>
<div class="home-preview__result"><span class="home-preview__status">PASS</span><span>PDF/UA-1</span></div>
<div class="home-preview__rule"><span>implemented checks</span><strong>all passed</strong></div>
</div>
</div>

</div>

## Built for trustworthy validation

<!-- prettier-ignore-start -->

<div class="grid cards home-card-grid" markdown>

- :material-human-wheelchair: **Accessibility**

    ***

    Check PDFs against PDF/UA-1 **accessibility requirements**.

- :material-file-check-outline: **PDF/A compliance**

    ***

    Validate PDF/A-1, PDF/A-2, and PDF/A-3 documents.

- :material-test-tube: **Corpus-tested**

    ***

    Compare behavior against the **veraPDF test corpus** and the **Isartor** test suite.

- :material-speedometer: **Fast**

    ***

    `page` is [2x to 10x faster than veraPDF](benchmark.md).

- :material-package-variant-closed: **Lightweight**

    ***

    Ships as an approximately ~8 MB binary, with **zero runtime requirements**.

- :material-application-brackets-outline: **Works anywhere**

    ***

    Use it from the [CLI](api/cli.md), [Rust](api/rust.md), [Python](api/python.md), or [WebAssembly](api/wasm.md).

</div>

<!-- prettier-ignore-end -->

## Get started

=== "CLI"

    ```console
    $ page document.pdf --profile ua1 --format details
    Result  : Non-conformant
    Rules   : 11 failed rules / 134 total
    Checks  : 24 failed checks
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

=== "Rust"

    You can use the `page_validation` crate to integrate into any existing Rust workflow:

    ```rust
    use page_validation::{ValidationOptions, ValidationProfile, validate_pdf};

    let options = ValidationOptions::default().profile(ValidationProfile::PdfUa1);
    let report = validate_pdf("file.pdf", &options)?;

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

=== "Python"

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

=== "WebAssembly"

    Read a PDF as a `Uint8Array` and validate it in the browser:

    ```ts
    import { ValidationProfile, validatePdfBytes } from "page-validation-wasm";

    const bytes = new Uint8Array(await pdfFile.arrayBuffer());
    const report = await validatePdfBytes(bytes, { profile: ValidationProfile.PDF_A_1B });

    if (report.isCompliant) {
        console.log("The document passed all implemented checks.");
    } else {
        for (const failure of report.failures) {
            console.log(`[${failure.ruleId}] ${failure.message}`);
        }
    }
    ```

</div>
