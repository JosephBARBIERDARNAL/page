#set document(
  title: "Example document",
  author: "page test suite",
  date: datetime(year: 2026, month: 1, day: 1),
)
#set page(paper: "a4", margin: 24mm)
#set text(lang: "en", size: 10pt)
#set heading(numbering: "1.")

= Example document

This intentionally ordinary PDF exercises text, vector graphics, and a link
annotation without declaring PDF/A conformance.

#rect(
  width: 100%,
  height: 18mm,
  fill: rgb("#d9e8f5"),
  stroke: 0.8pt + rgb("#285f8f"),
  radius: 2pt,
)

See the #link("https://verapdf.org/")[veraPDF website] for the reference
validator used by the test suite.
