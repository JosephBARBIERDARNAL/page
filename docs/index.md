# page

A fast and lightweight PDF accessibility and compliance checker. You can currently use it as a [CLI](api/cli.md) or via [Rust](api/rust.md).

## Features

- **Accessibility**: check PDFs against PDF/UA-1 accessibility requirements
- **PDF/A compliance**: validate PDF/A-1, PDF/A-2, and PDF/A-3 documents
- **Tested**: uses the same test corpus as _veraPDF_ :material-information-outline:{ title="veraPDF is the gold standard of PDF/A validation, which is why it's used as a reference." }
- **Fast**: preliminary tests suggest that `page` is 3x to 15x faster than veraPDF
- **Lightweight**: ~9 MB binary, with peak RSS 3x to 8x lower than veraPDF

## Roadmap

So far, all PDF/A and PDF/UA-1 formats have been implemented. The current focus is on consolidating those, making sure they are heavily tested against a large corpus of documents. The goal is to have those foundations completed by **the end of 2026**.
