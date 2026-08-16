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
      
      page implements preliminary PDF/A-1, PDF/A-2, and PDF/A-3 validators based on ISO 19005-1, ISO 19005-2, and ISO 19005-3, with differential verification against veraPDF 1.30.2 and documented coverage gaps around the impractical maximum indirect-object boundary.

- [x] PDF/A-1b
- [x] PDF/A-1a
- [x] PDF/A-2a
- [x] PDF/A-2b
- [x] PDF/A-2u
- [x] PDF/A-3a
- [x] PDF/A-3b
- [x] PDF/A-3u
- [ ] PDF/A-4
- [ ] PDF/A-4e
- [ ] PDF/A-4f
- [ ] PDF/UA-1
- [ ] PDF/UA-2
