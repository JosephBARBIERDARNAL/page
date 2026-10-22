---
hide:
  - navigation
  - toc
---

<div class="home-page" markdown>

<div class="home-hero" markdown>

<div class="home-hero__content" markdown>

# Fast PDF accessibility & compliance validation

Check PDF documents against accessibility and compliance requirements with a Rust-based engine available as a CLI, Rust crate, Python package, or browser JavaScript package.

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

- :material-file-check-outline: **Archiving**

    ***

    Validate PDF/A-1, PDF/A-2, and PDF/A-3 documents.

- :material-test-tube: **veraPDF compatible**

    ***

    Matches **veraPDF** behavior against the **veraPDF test corpus** as well as the **Isartor** test suite.

- :material-speedometer: **Fast**

    ***

    `page` is an [order of magnitude faster than veraPDF](benchmark.md).

- :material-package-variant-closed: **Lightweight**

    ***

    Ships as an approximately ~8 MB binary, with **zero runtime requirements**.

- :material-application-brackets-outline: **Works anywhere**

    ***

    Use it from the [CLI](api/cli.md), [Rust](api/rust.md), [Python](api/python.md), or [JavaScript](api/javascript.md).

</div>

<!-- prettier-ignore-end -->

## Get started

=== "CLI"

    ```console
    $ page document.pdf --profile ua1 --format details
    Result  : Non-conformant
    Rules   : 2 failed rules / 104 total
    Profile : PDF/UA-1
    Time    : 0.017s

    [ISO 14289-1:2014:5:1]
      Metadata: XMP does not contain the PDF/UA Identification schema
      Possible cause: The PDF/UA identification metadata is absent, malformed, or inconsistent with PDF/UA-1.
      Suggested fix: Correct the PDF/UA identification properties in the exported XMP metadata.

    [ISO 14289-1:2014:5:2]
      Metadata: XMP declares PDF/UA part values ["2"], expected exactly one value 1
      Possible cause: The PDF/UA identification metadata is absent, malformed, or inconsistent with PDF/UA-1.
      Suggested fix: Correct the PDF/UA identification properties in the exported XMP metadata.
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
            if let Some(guidance) = &failure.guidance {
                eprintln!("Possible cause: {}", guidance.possible_cause);
                eprintln!("Suggested fix: {}", guidance.suggested_fix);
            }
        }
    }
    ```

=== "Python"

    You can use the `page-validation` Python package to integrate into any existing Python workflow:

    ```py
    import page

    report = page.validate_pdf("document.pdf", profile="ua1")

    if report.is_compliant:
        print("The document passed all implemented checks.")
    else:
        for failure in report.failures:
            print(f"[{failure.rule_id}] {failure.message}")
            if failure.guidance is not None:
                print(f"Possible cause: {failure.guidance.possible_cause}")
                print(f"Suggested fix: {failure.guidance.suggested_fix}")
    ```

=== "JavaScript"

    Read a PDF as a `Uint8Array` and validate it in the browser with the WebAssembly-powered JavaScript package:

    ```ts
    import { ValidationProfile, validatePdfBytes } from "page-validation-wasm";

    const bytes = new Uint8Array(await pdfFile.arrayBuffer());
    const report = await validatePdfBytes(bytes, { profile: ValidationProfile.PDF_UA_1 });

    if (report.isCompliant) {
        console.log("The document passed all implemented checks.");
    } else {
        for (const failure of report.failures) {
            console.log(`[${failure.ruleId}] ${failure.message}`);
            if (failure.guidance !== null) {
                console.log(`Possible cause: ${failure.guidance.possibleCause}`);
                console.log(`Suggested fix: ${failure.guidance.suggestedFix}`);
            }
        }
    }
    ```

</div>
