#set document(
  title: "Validation proposal",
  author: "page test suite",
  date: datetime(year: 2026, month: 1, day: 1),
)
#set page(paper: "us-letter", margin: 1in)
#set text(lang: "en", size: 10pt)
#set par(justify: true)
#set heading(numbering: "1.")

= Validation proposal

This proposal is an ordinary Typst-generated PDF. It intentionally has no
PDF/A conformance target so both validators can exercise their noncompliant
document paths.

== Scope

- Metadata inspection
- Embedded-font inspection
- Colour and graphical-content inspection
- Link annotation inspection

The reference implementation is #link("https://verapdf.org/")[veraPDF].

== Expected result

The file parses successfully and fails the selected PDF/A-1b profile.
