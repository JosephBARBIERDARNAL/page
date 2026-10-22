# page

A modern PDF checker for accessibility and archival conformance that is <ins>accurate</ins>, *user-friendly*, and `fast`, written in Rust.

- [Documentation](https://josephbarbierdarnal.github.io/page/)
- [Installation](#installation)
- [Usage](#quick-start)

`page` is a new, independent **validator for PDF documents**, including PDF/UA (accessibility) and PDF/A (archiving). It passes the veraPDF corpus test suite, as well as the Isartor test suite. It's an [**order of magnitude faster**](https://josephbarbierdarnal.github.io/page/benchmark/) than veraPDF with **zero runtime requirements**.

```console
$ page document.pdf --profile ua1 --format details
Result  : Non-conformant
Rules   : 2 failed rules / 104 total
Profile : PDF/UA-1
Time    : 0.017s

[ISO 14289-1:2014:5:1]
  Metadata: XMP does not contain the PDF/UA Identification schema
  Possible cause: The PDF/UA identification metadata is absent/malformed/inconsistent
  Suggested fix: Correct the PDF/UA identification properties in the XMP metadata

[ISO 14289-1:2014:6.2:1]
  Conformance: the document catalog MarkInfo dictionary must contain boolean /Marked true
  Possible cause: The catalog does not clearly mark the file as a tagged PDF.
  Suggested fix: Enable tagged PDF output and set MarkInfo/Marked to true.
```

> [!TIP]
> `page` can be used from [Rust](https://josephbarbierdarnal.github.io/page/api/rust/), [Python](https://josephbarbierdarnal.github.io/page/api/python/) and [JavaScript](https://josephbarbierdarnal.github.io/page/api/javascript/).

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

## veraPDF compatibility

> [!IMPORTANT]
> `page` validates PDF/A (from 1 to 3) and PDF/UA-1 only, while `veraPDF` also supports other profiles such as PDF/A-4 or PDF/UA-2.

`veraPDF` has and uses a [test corpus](https://github.com/veraPDF/veraPDF-corpus) with ~2000 PDF files (+ an additional 200 PDF files for the [Isartor test suite](https://pdfa.org/resource/isartor-test-suite/)). For each of these files, there is an expected failed rule that must be raised. Right now, `page` **passes all of those tests**. If you look in the CI, you'll find a job called _"veraPDF test corpus / veraPDF corpus gate"_ that runs those tests against `page` on every PR/push.

A less formal proof of compatibility is that I now personally use `page` as my default PDF validator on a daily basis, and `veraPDF` to verify the results; the **output is basically the same all the time**.

<br>

## License

`page`'s original source code and all other project-authored material are licensed under the **MIT License**. Everything not expressly identified as third-party or reference material in the accompanying [THIRD_PARTY_NOTICES.md](./crates/page_validation/THIRD_PARTY_NOTICES.md) is MIT. The `page_validation` crate additionally includes Adobe CMap Resources under BSD-3-Clause and adapted Mozilla PDF.js encoding tables under Apache-2.0. See the notice and license files for details.
