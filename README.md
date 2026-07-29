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

The repository is a Cargo workspace with two packages:

- `tag-validation` contains the reusable parser, normalized model, validation rules, reports, safety limits, and veraPDF differential engine.
- `tag-cli` contains client-side argument parsing, output selection, process exit behavior, and the `tag` and `verapdf-diff` executables. It depends on `tag-validation` through a workspace path dependency.

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

