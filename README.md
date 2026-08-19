# page

`page` is an <u>experimental</u> PDF/A and PDF/UA validation engine written in Rust. It can currently be used either as a CLI or as a Rust crate.

<br>

## Installation

- macOS/Linux

```sh
curl -fsSL https://github.com/josephbarbierdarnal/page/releases/download/v0.3.1/page_cli-installer.sh | sh
```

- PowerShell

```sh
irm https://github.com/josephbarbierdarnal/page/releases/download/v0.3.1/page_cli-installer.ps1 | iex
```

- Cargo (from source)

```sh
cargo install --git https://github.com/josephbarbierdarnal/page.git page_cli --bin page
```

<br>

## Usage

- [CLI](https://josephbarbierdarnal.github.io/page/api/cli/)
- [Rust](https://josephbarbierdarnal.github.io/page/api/rust/)

<br>

## License

The original `page` source code is licensed under [MIT](LICENSE).

The `page_validation` crate bundles Adobe CMap Resources under the BSD 3-Clause license; see the [third-party notices](crates/page_validation/THIRD_PARTY_NOTICES.md). Binary distributions must include both license documents.
