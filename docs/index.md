# page

`page` is an <span class="pretty-highlight">experimental</span> PDF/A and PDF/UA validation engine written in Rust.

You can currently use it as a [CLI](api/cli.md) or via [Rust](api/rust.md)

## Features

- **Compliant**: validate all PDF/A-1, PDF/A-2, PDF/A-3 and PDF/UA-1 formats
- **Tested**: uses the same test corpus as _veraPDF_ :material-information-outline:{ title="veraPDF is the gold standard of PDF/A validation, which is why it's used as a reference." }
- **Fast**: preliminary tests suggest that `page` is between 3x to 15x faster than veraPDF
- **Lightweight**: binary size is ~9MB and peak RSS is between 3x to 8x lower than veraPDF

## Roadmap

So far, all PDF/A and PDF/UA-1 formats have been implemented. The current focus is on consolidating those, making sure they are heavily tested against a large corpus of documents. The goal is to have those foundations completed by **the end of 2026**.
