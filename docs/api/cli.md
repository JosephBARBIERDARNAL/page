---
title: "CLI"
---

Validate one PDF against a profile:

```sh
page document.pdf --profile a-1b
```

Add `--format details` to emit the details of the failure:

```sh
page document.pdf --profile a-1b --format details
```

Or `--format json` to emit the details as JSON:

```sh
page document.pdf --profile a-1b --format json
```
