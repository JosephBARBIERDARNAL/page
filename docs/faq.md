---
title: "FAQ"
---

## Related projects

There are multiple projects that attempt to provide a PDF validator, but only one is:

- open source
- fully compliant with ISO standards
- cross-platform
- non-commercial

This project is [veraPDF](https://verapdf.org/). It is, by far, the **best option available** out there. Other options such as [PAC](https://pac.pdf-accessibility.org/en) or [Adobe Acrobat](https://www.adobe.com/) meet only a subset of those criteria.

`veraPDF` supports validation of all PDF/A (long-term archiving) and PDF/UA (universal accessibility) formats. It might not seem so, but this is a very complicated task. You can understand why that is and how `veraPDF` does it in this great [blog post](https://pdfa.org/how-verapdf-does-pdfa-validation/).

!!! warning

      Some open-source software claims to provide PDF validation, but this claim is often, if not always, misleading. Those tools often check only a very small subset of all the rules for a given profile.

## How does `page` compare to `veraPDF`?

Currently, `page` is **far from a usable state**: it does not provide complete profile validations for most PDF profiles and has no stable API.

Even if `page` is meant as an "alternative" to `veraPDF`, **`page` only exists because of `veraPDF`**. The latter is the most important development tool used for `page`, as it provides the source of truth for which rules pass or fail for each PDF profile.

In the long run, I hope that `page` will provide a veraPDF-compliant alternative that is faster and more lightweight. [Latest measurements](./#performance) suggest that `page` could be around an order of magnitude faster and use correspondingly less memory (peak RSS).

!!! info

      The reason for those differences is that `page` is written in **Rust** and `veraPDF` in **Java**. Note that it is likely that a significant part of `veraPDF`'s memory usage is due to the JVM, not `veraPDF` itself.

      The reasons for choosing Rust for `page` are speed, memory and type safety, and downstream integration. Indeed, it's easier to integrate Rust-based programs with Python and Node.js, as well as with other languages through a C interface (even if that's also possible with Java).

## Why are there multiple PDF formats?

TODO

## What is PDF validation?

TODO
