---
title: "Installation"
---

`page` distribute pre-built binaries for macOS/Linux/Windows. If you're a Rust hacker, you can also install the latest dev version via Cargo.

!!! tip "Info"

      Those are instructions for installing the **CLI**. If you want to integrate `page` in your Rust workflows, check out the [page crate](./api/rust/quick-start.md).

### macOS/Linux

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/josephbarbierdarnal/page/releases/download/v0.4.0/page_cli-installer.sh | sh
```

### PowerShell

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/josephbarbierdarnal/page/releases/download/v0.4.0/page_cli-installer.ps1 | iex"
```

### Cargo

```sh
cargo install --git https://github.com/josephbarbierdarnal/page.git page_cli --bin page
```
