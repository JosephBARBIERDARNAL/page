# tag

`tag` is an experimental PDF/A and PDF/UA validator, written in Rust.

This project is very early stage, and current focus is on the PDF/A-1b validation.

The validator remains preliminary until the
[PDF/A-1B completion gate](docs/reference/pdfa1b-completion.md) passes. A local
pass is not currently a PDF/A-1B conformance claim.

PDF/A-1B file syntax and low-level COS-object validation are complete against
the pinned veraPDF 1.28.2 profile; higher-level profile coverage remains in
development.

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

Compare end-to-end command-line performance with a veraPDF executable (release
builds are used for `tag`):

```sh
just benchmark verapdf=/path/to/verapdf files="document.pdf"
```

The benchmark starts each validator for every measured run, so its timings
include process startup, file I/O, parsing, validation, and report generation.

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

[MIT](LICENSE)
