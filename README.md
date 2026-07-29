# tag

`tag` is an experimental PDF/A and PDF/UA validator, written in Rust.

This project is very early stage, and current focus is on the PDF/A-1b validation.

<br>

## Install

With Rust and Cargo installed:

```sh
cargo install --git https://github.com/josephbarbierdarnal/tag.git tag-cli --bin tag
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

The JSON API is intentionally small and stable. It contains `file`, `profile`,
`valid`, and `failures`; parser and operational failures use an `error` object
instead of a conformance-failure list. It is not veraPDF's report format.

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
