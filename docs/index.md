# page

`page` is an <span class="pretty-highlight">experimental</span> PDF/A and PDF/UA validation engine written in Rust. It can currently be used either as a **CLI** or as a **Rust crate**.

## Installation

=== "macOS/Linux"

      ```sh
      curl -fsSL https://github.com/josephbarbierdarnal/page/releases/download/v0.4.0/page_cli-installer.sh | sh
      ```

=== "PowerShell"

      ```sh
      irm https://github.com/josephbarbierdarnal/page/releases/download/v0.4.0/page_cli-installer.ps1 | iex
      ```

=== "Cargo"

      ```sh
      cargo install --git https://github.com/josephbarbierdarnal/page.git page_cli --bin page
      ```

## Usage

- [CLI](api/cli.md)
- [Rust](api/rust.md)

## Roadmap

Current focus is on **PDF/UA-1**, see the [reference issue](https://github.com/JosephBARBIERDARNAL/page/issues/61).

- [x] PDF/A-1a
- [x] PDF/A-1b
- [x] PDF/A-2a
- [x] PDF/A-2b
- [x] PDF/A-2u
- [x] PDF/A-3a
- [x] PDF/A-3b
- [x] PDF/A-3u
- [ ] PDF/UA-1
- [ ] PDF/A-4
- [ ] PDF/A-4e
- [ ] PDF/A-4f
- [ ] PDF/UA-2
