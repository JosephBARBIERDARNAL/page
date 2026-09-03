Thanks for taking the time to improve `page`. This guide covers the local setup and the main checks to run before opening a pull request.

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
- [`uv`](https://docs.astral.sh/uv/) if you want to preview the documentation website
- (optional but recommended) [`just`](https://github.com/casey/just) for running project tasks, see `justfile` file

## Install veraPDF

Download the [veraPDF 1.30.2 Greenfield installer](https://software.verapdf.org/rel/1.30/), extract it, and run `verapdf-install` on macOS/Linux or `verapdf-install.bat` on Windows.

Put the installed CLI on `PATH`, or point the project to it explicitly:

```bash
export VERAPDF_BIN=/path/to/verapdf/bin/verapdf
"$VERAPDF_BIN" --version
```

The version must be 1.30.x; 1.30.2 is recommended.

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
