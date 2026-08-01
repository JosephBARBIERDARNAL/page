# tag

`tag` is an experimental PDF/A and PDF/UA validator, written in Rust.

> [!WARNING]
> This project is **very early stage**, and current focus is on the PDF/A-1b validation.

<br>

## Install

With Rust and Cargo installed:

```sh
cargo install --git https://github.com/josephbarbierdarnal/tag.git tag_cli --bin tag
```

`tag` is installed to `~/.cargo/bin`; add that directory to your `PATH` if needed.

<br>

## Usage

Validate one PDF against a profile:

```sh
tag document.pdf --profile a-1b
```

Add `--json` to emit the validation report as JSON:

```sh
tag document.pdf --profile a-1b --json
```

<br>

## Roadmap

- [ ] PDF/A-1b (current WIP)
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

<br>

## License

The original `tag` source code is licensed under [MIT](LICENSE).

The `tag_validation` crate bundles Adobe CMap Resources under the BSD 3-Clause license;
see the [third-party notices](crates/tag_validation/THIRD_PARTY_NOTICES.md).
Binary distributions must include both license documents.
