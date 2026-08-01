# page

`page` is an experimental PDF/A and PDF/UA validator, written in Rust.

!!! warning

      `page` is an experimental project and isn't already usable. Current focus is on validating PDF/A-1b.

## Install

With Rust and Cargo installed:

```sh
cargo install --git https://github.com/josephbarbierdarnal/page.git page_cli --bin page
```

`page` is installed to `~/.cargo/bin`; add that directory to your `PATH` if needed.

## Usage

- [CLI](api/cli.md)
- [Rust](api/rust.md)
- [Python](api/python.md)
- [Node.js](api/nodejs.md)

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


## Performance

`page` is still in a preliminary stage, but current tests and benchmark I made (only on PDF/A-1b) give:

| Metric | `page` | `veraPDF` | ratio
|---|---|---|---
| Validation time (median) | ~50 ms | ~1300 ms | ~26× faster
| Peak RSS (median) | ~13 MB | ~254 MB | ~20× lighter

Benchark code lives in [bench](https://github.com/JosephBARBIERDARNAL/page/tree/main/bench).
