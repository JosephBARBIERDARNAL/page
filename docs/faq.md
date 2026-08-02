---
title: "FAQ"
---

## What is PDF validation?

TODO

## Related projects

There are multiple projects that attempt to provide a PDF validator, but only one is:

- open source
- fully respect ISO conformance
- cross-platform
- non-commercial

This project is [veraPDF](https://verapdf.org/). It is, by far, the **best option available** out there. Other options such as [PAC](https://pac.pdf-accessibility.org/en) or [Adobe Acrobat](https://www.adobe.com/) only respect a subset of those criteria.

`veraPDF` supports validation of all PDF/A (long term support) and PDF/UA (universal access) formats. It might not seem so, but this is a very complicated thing. You can understand why is that, and how `veraPDF` does it in this great [blog post](https://pdfa.org/how-verapdf-does-pdfa-validation/).

!!! warning

      Some open source softwares suggest that they provide PDF validation, but it's often, if not all the time, not really true. Those tools often just check for a, very small, subset of all rules for a given profile.

## How `page` compares to `veraPDF`?

Currently, `page` is **far from a usable state**: it does not provide complete profile validations for most PDF profiles and has no stable API.

Even if `page` is meant as an "alternative" to `veraPDF`, **`page` only exists because of `veraPDF`**. The latter is the most important development tool used for `page` as it provides the source of truth for all passed/failed rules for each PDF profile.

In the long run, I hope that `page` will provide a veraPDF-compliant alternative, that is faster and very lightweight. [Latest measurements](./#performance) suggest that `page` could be around an order of magnitude faster and use correspondingly less memory (peak RSS).

!!! info
      
      Reasons for those difference is that `page` is written in Rust and `veraPDF` in Java. It is likely that memory usage from `veraPDF` is due to the JVM.

      The reasons of choosing Rust for `page` are: speed, memory/type safety and downstream integration. Indeed, it's easier to integrat Rust-based programs in other languages like Python, Node.js and other languages with a C interface.

## Why are there multiple PDF formats?

TODO
