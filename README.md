# page

A fast PDF accessibility and conformance checker.

- [Documentation](https://josephbarbierdarnal.github.io/page/)
- [Installation](#installation)
- [Usage](#quick-start)

`page` is a new, independent **Rust-based validator for PDF documents**, including PDF/UA (accessibility) and PDF/A (archiving). It passes the veraPDF corpus test suite, as well as the Isartor test suite. It's between [**3x to 12x faster**](https://josephbarbierdarnal.github.io/page/benchmark/) than veraPDF with **zero runtime requirements**.

<br>

## Installation

`page` distributes pre-built binaries for macOS/Linux/Windows:

### macOS/Linux

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/josephbarbierdarnal/page/releases/download/v0.11.0/page_cli-installer.sh | sh
```

### PowerShell

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/josephbarbierdarnal/page/releases/download/v0.11.0/page_cli-installer.ps1 | iex"
```

<br>

## Quick start

- Check accessibility (PDF/UA-1) compliance:

```console
$ page document.pdf --profile ua1
Result  : Non-conformant
Profile : PDF/UA-1
Time    : 0.052s
```

- Find reasons for non-conformance with `--format details`

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

> [!NOTE]
> `[.........]` here are just placeholders for the actual messages

`page` can also be used from [Rust](https://josephbarbierdarnal.github.io/page/api/rust/), [Python](https://josephbarbierdarnal.github.io/page/api/python/) and [JavaScript](https://josephbarbierdarnal.github.io/page/api/javascript/).

<br>

## veraPDF compatibility

> [!IMPORTANT]
> `page` validates PDF/A (from 1 to 3) and PDF/UA-1 only, while `veraPDF` also supports other profiles such as PDF/A-4 or PDF/UA-2.

`veraPDF` has and uses a [test corpus](https://github.com/veraPDF/veraPDF-corpus) with ~2000 PDF files (+ an additional 200 PDF files for the [Isartor test suite](https://pdfa.org/resource/isartor-test-suite/)). For each of these files, there is an expected failed rule that must be raised. Right now, `page` **passes all of those tests**. If you look in the CI, you'll find a job called _"veraPDF test corpus / veraPDF corpus gate"_ that runs those tests against `page` on every PR/push.

A less formal proof of compatibility is that I now personally use `page` as my default PDF validator on a daily basis, and `veraPDF` to verify the results; the **output is basically the same all the time**.

<br>

## License

`page`'s original source code and all other project-authored material are licensed under the **MIT License**. Everything not expressly identified as third-party or reference material in the accompanying [THIRD_PARTY_NOTICES.md](./crates/page_validation/THIRD_PARTY_NOTICES.md) is MIT. The `page_validation` crate additionally includes Adobe CMap Resources under BSD-3-Clause and adapted Mozilla PDF.js encoding tables under Apache-2.0. See the notice and license files for details.
