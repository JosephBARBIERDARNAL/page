# page

`page` is an <u>experimental</u> PDF/A and PDF/UA validation engine, written in Rust. It can currently be used either as a CLI or as a Rust crate.

- [Documentation](https://josephbarbierdarnal.github.io/page/)
- [Installation](https://josephbarbierdarnal.github.io/page/#installation)
- Usage
  - [CLI](https://josephbarbierdarnal.github.io/page/api/cli/)
  - [Rust](https://josephbarbierdarnal.github.io/page/api/rust/)

> [!NOTE]
> The engine currently implements PDF/A-1, PDF/A-2 and PDF/A-3 validation. The focus is currently on **PDF/UA-1**.

<br>

## License

The original `page` source code is licensed under [MIT](LICENSE).

The `page_validation` crate bundles Adobe CMap Resources under the BSD 3-Clause license; see the [third-party notices](crates/page_validation/THIRD_PARTY_NOTICES.md). Binary distributions must include both license documents.
