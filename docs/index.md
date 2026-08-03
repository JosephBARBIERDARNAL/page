# page

`page` is an experimental PDF/A and PDF/UA validaton engine, written in Rust.

!!! warning

      `page` is an experimental project and isn't usable yet. The current focus is on validating PDF/A-1b.

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

- [ ] PDF/A-1b (~99% done)
      
      - complete annotations, actions, forms, and interactive-feature validation

- [ ] PDF/A-1a
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
