This guide covers the local setup and the main checks to run before opening a pull request.

**Before making changes, comment on an existing issue or open a new one.**

## Fork and clone

Fork the repository on GitHub, then clone your fork locally:

```bash
git clone git@github.com:<your-username>/page.git
cd page
git remote add upstream https://github.com/josephbarbierdarnal/page.git
git fetch upstream
```

Create a branch for your change:

```bash
git checkout -b <short-description>
```

## Prerequisites

Install these tools before setting up the project:

- `Git`
- `Cargo` and Rust >= 1.88.0
- [`uv`](https://docs.astral.sh/uv/) for Python bindings and if you want to preview the documentation website
- [`bun`](https://bun.com/) for Wasm bindings
- (optional but recommended) [`just`](https://github.com/casey/just) for running project tasks, see `justfile` file

## Install dependencies

Fetch the locked Rust dependencies:

```bash
cargo fetch --locked
```

## Run unit tests

Run the full workspace test suite:

```bash
just test
```

## Run corpus tests

Run the corpus conformance gate. The first run downloads a sparse checkout of the pinned veraPDF corpus:

```bash
just verapdf-corpus
```

## Formatting and checks

Run formatting and lint checks with warnings treated as errors:

```bash
just fmt && just lint
```

## Documentation

Serve the documentation locally with:

```bash
just preview
```

Then open `http://localhost:8000` to preview the documentation website.
