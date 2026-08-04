# page

`page` is an experimental PDF/A and PDF/UA validaton engine, written in Rust.

!!! warning

      `page` is an experimental project and isn't really usable yet.

## Installation

With Rust and Cargo installed:

```sh
cargo install --git https://github.com/pagestandards/page.git page_cli --bin page
```

`page` is installed to `~/.cargo/bin`; add that directory to your `PATH` if needed.

## Usage

- [CLI](api/cli.md)
- [Rust](api/rust.md)
- [Python](api/python.md)
- [Node.js](api/nodejs.md)
- [WASM](api/web-assembly.md)

## Roadmap

!!! failure "Important"
      
      page implements a PDF/A-1 validator based on ISO 19005-1 and is extensively verified against veraPDF 1.30.2, with documented limitations around the impractical [maximum indirect-object boundary](https://github.com/veraPDF/veraPDF-validation-profiles/wiki/PDFA-Part-1-rules#rule-6112-7).

- [x] PDF/A-1b
- [x] PDF/A-1a
- [ ] PDF/A-2a
- [ ] PDF/A-2b
- [ ] PDF/A-2u
- [ ] PDF/A-3a
- [ ] PDF/A-3b
- [ ] PDF/A-3u
- [ ] PDF/A-4
- [ ] PDF/A-4e
- [ ] PDF/A-4f
- [ ] PDF/UA-1
- [ ] PDF/UA-2
