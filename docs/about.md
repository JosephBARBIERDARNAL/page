---
title: About
---

## What does `page` do?

`page` is, at its core, a PDF validation engine. In more practical terms, it verifies that a given PDF file complies with certain standards. For example, for a PDF to be considered accessible, it is often required to be PDF/UA-1 compliant.

Since the creation of the PDF format more than 30 years ago, many different variants have appeared, each with a different purpose. For example, all PDF/A formats (around 10) focus on archival preservation, ensuring that a PDF created in the 2000s will still open decades from now, while PDF/UA formats (UA-1 and UA-2) focus on accessibility, ensuring that everyone, including people with visual disabilities, can use a PDF.

`page` aims to make that verification easy, fast, and free.

## How it works?

When you give `page` a PDF, it reads the file and parses its structure into a **normalized document model**. This model brings together the PDF object graph and the information needed for validation, such as metadata, XMP declarations, output intents, fonts, pages, annotations, forms, and content streams. Parsing is done via [lopdf](https://github.com/J-F-Liu/lopdf), and is kept separate from conformance checks, so malformed input or resource-limit failures can be reported distinctly from a PDF that simply violates a standard.

Next, `page` selects the validation profile: it uses the profile passed on the command line, or infers one from the PDF's XMP identification metadata when no profile is specified. A document is compliant only when **every rule** implemented for the selected profile passes.

Internally, `page` has a "fail fast" mode, which lets us be around 2x to 5x faster depending on the document. Instead of checking for every possible rule, it stops at the very first one that is violated, and only all rules checked when it's required. This feature is automatically used when possible, and only relevant when the document isn't compliant.

## What are the alternatives?

There are multiple projects that attempt to provide a PDF validator, but only one is:

- open source
- based on ISO standards
- cross-platform
- non-commercial

This project is [veraPDF](https://verapdf.org/). It is, by far, the **best option available** out there. Other options such as [PAC](https://pac.pdf-accessibility.org/en) or [Adobe Acrobat](https://www.adobe.com/) meet only a subset of those criteria.

`veraPDF` supports validation of all PDF/A (long-term archiving) and PDF/UA (universal accessibility) formats. It might not seem so, but this is a <span class="pretty-highlight">very complicated task</span>. You can understand why that is and how `veraPDF` does it in this great [blog post](https://pdfa.org/how-verapdf-does-pdfa-validation/).

`page` uses heavily `veraPDF` as the source of truth to validate or not if what `page` does is correct.

## Why are there multiple PDF formats?

The PDF specification is intentionally broad: it supports features ranging from interactive forms and multimedia to encryption, JavaScript, and digital signatures. This flexibility makes PDF suitable for many use cases, but it also makes it impossible to guarantee that every PDF will behave consistently across all software and over long periods of time.

To address this, the ISO standard defines a number of **specialized PDF profiles**. Each profile restricts or requires certain features to guarantee a particular property.

Some of the most common profiles are:

- `PDF/A`: Long-term document preservation and archiving.
- `PDF/UA`: Universal accessibility for users of assistive technologies.
- `PDF/X`: Reliable printing and graphic arts workflows.
- `PDF/E`: Engineering and technical documentation.
- `PDF/VT`: Variable-data and transactional printing.

These profiles are not competing formats. Instead, they are **subsets (and sometimes supersets) of the PDF specification**, each designed for a specific purpose.

For example, PDF/A forbids features such as JavaScript or external dependencies because they could make a document impossible to reproduce decades later. Conversely, PDF/UA requires additional semantic information (such as a tag tree and alternative text) to ensure documents are accessible to screen readers.

Because each profile defines **hundreds of requirements**, validating a PDF is much more involved than simply checking whether it "opens correctly." A validator must verify that every applicable rule is satisfied for the requested profile.
