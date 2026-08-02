# page

`page` is an experimental PDF/A and PDF/UA validator, written in Rust.

> [!WARNING]
> This project is **very early stage**, and current focus is on the PDF/A-1b validation.

<br>

## Install

With Rust and Cargo installed:

```sh
cargo install --git https://github.com/josephbarbierdarnal/page.git page_cli --bin page
```

`page` is installed to `~/.cargo/bin`; add that directory to your `PATH` if needed.

<br>

## Usage

Validate one PDF against a profile:

```sh
page document.pdf --profile a-1b
```

By default, `page` prints a compact validation summary. Use detailed output to
list every failed rule:

```sh
page document.pdf --profile a-1b --format details
```

Or emit the validation report as JSON:

```sh
page document.pdf --profile a-1b --format json
```

Human-readable output uses colors when writing to a terminal. Set the
`NO_COLOR` environment variable or pass `--no-color` to disable them.

<br>

## Roadmap

- [ ] PDF/A-1b (~95% done, current WIP)
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

## Performance

`page` is still in a preliminary stage, but current tests and benchmark I made (only on PDF/A-1b) give:

| Metric | `page` | `veraPDF` | ratio
|---|---|---|---
| Validation time (median) | ~50 ms | ~1300 ms | ~26× faster
| Peak RSS (median) | ~13 MB | ~254 MB | ~20× lighter

Benchark code lives in [bench](./bench/).

<br>

## License

The original `page` source code is licensed under [MIT](LICENSE).

The `page_validation` crate bundles Adobe CMap Resources under the BSD 3-Clause license;
see the [third-party notices](crates/page_validation/THIRD_PARTY_NOTICES.md).
Binary distributions must include both license documents.
