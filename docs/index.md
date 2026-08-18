# page

`page` is an <u>experimental</u> PDF/A and PDF/UA validation engine written in Rust. It can currently be used either as a CLI or as a Rust crate.

## Installation

With Rust and Cargo installed:

```sh
cargo install --git https://github.com/josephbarbierdarnal/page.git page_cli --bin page
```

`page` is installed to `~/.cargo/bin`; add that directory to your `PATH` if needed.

## Usage

- [CLI](api/cli.md)
- [Rust](api/rust.md)

## Roadmap

!!! failure "Important"

      page implements PDF/A-1, PDF/A-2, and PDF/A-3 validators based on ISO 19005-1, ISO 19005-2, and ISO 19005-3, with differential verification against veraPDF 1.30.2. A crossed checkbox means that the core of the rule has been implemented, but its behavior does not yet fully match veraPDF.

- [x] PDF/A-1a
- [x] PDF/A-1b
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
