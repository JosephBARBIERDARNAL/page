# page

`page` is an <u>experimental</u> PDF/A and PDF/UA validation engine written in Rust. It can currently be used either as a CLI or as a Rust crate.

<br>

## Installation

Prebuilt binaries for macOS, Linux, and Windows are available from the [latest release](https://github.com/josephbarbierdarnal/page/releases/latest).

With Rust and Cargo installed:

```sh
cargo install --git https://github.com/josephbarbierdarnal/page.git page_cli --bin page
```

`page` is installed to `~/.cargo/bin`; add that directory to your `PATH` if needed.

<br>

## Usage

- [CLI](https://josephbarbierdarnal.github.io/page/api/cli/)
- [Rust](https://josephbarbierdarnal.github.io/page/api/rust/)

<br>

## License

The original `page` source code is licensed under [MIT](LICENSE).

The `page_validation` crate bundles Adobe CMap Resources under the BSD 3-Clause license; see the [third-party notices](crates/page_validation/THIRD_PARTY_NOTICES.md). Binary distributions must include both license documents.
