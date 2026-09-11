---
hide:
  - navigation
  - toc
---

<div class="home-page" markdown>

<div class="home-hero" markdown>

<div class="home-hero__content" markdown>

<div class="home-eyebrow">PDF accessibility and validation</div>

# Fast, lightweight PDF validation

Check PDF documents against accessibility and compliance requirements with a small Rust-based engine that runs as a CLI, library, Python package, or WebAssembly module.

[Get started](installation.md){ .md-button .md-button--primary }
[Try the demo](demo.md){ .md-button }

<div class="home-hero__meta">
<span>Very fast</span>
<span>No runtime required</span>
<span>Runs in the browser</span>
</div>

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

    Check PDFs against PDF/UA-1 accessibility requirements.

- :material-file-check-outline: **PDF/A compliance**

    ***

    Validate PDF/A-1, PDF/A-2, and PDF/A-3 documents.

- :material-test-tube: **Corpus-tested**

    ***

    Compare behavior against the veraPDF test corpus.

- :material-speedometer: **Fast**

    ***

    Preliminary tests suggest that `page` is [3x to 15x faster than veraPDF](benchmark.md).

- :material-package-variant-closed: **Lightweight**

    ***

    Ships as an approximately 8 MB binary with peak RSS 3x to 8x lower than veraPDF.

- :material-application-brackets-outline: **Works anywhere**

    ***

    Use it from the CLI, Rust, Python, or WebAssembly.

</div>

<!-- prettier-ignore-end -->

## One engine, four ways to use it

Choose the interface that fits your workflow. All bindings use the same validation engine.

<!-- prettier-ignore-start -->

<div class="grid cards home-interface-grid" markdown>

- :material-console: **CLI**

    ***

    Validate files directly from your terminal.

    [:octicons-arrow-right-24: CLI usage](api/cli.md)

- :material-language-rust: **Rust crate**

    ***

    Embed validation in Rust applications and services.

    [:octicons-arrow-right-24: Rust usage](api/rust.md)

- :material-language-python: **Python package**

    ***

    Add PDF validation to Python workflows and tooling.

    [:octicons-arrow-right-24: Python usage](api/python.md)

- :material-web: **WebAssembly**

    ***

    Run validation locally in web applications and browsers.

    [:octicons-arrow-right-24: WebAssembly usage](api/wasm.md)

</div>

<!-- prettier-ignore-end -->

</div>
