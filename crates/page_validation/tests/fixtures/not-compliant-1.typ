#set document(
  title: "Non-compliant example",
  author: "page test suite",
  date: datetime(year: 2026, month: 1, day: 1),
)
#set page(paper: "a4", margin: 24mm, numbering: "1")
#set text(lang: "en", size: 10pt)
#set heading(numbering: "1.")

= Non-compliant example

This document is generated without a PDF/A target. It is a valid PDF, but it
intentionally makes no PDF/A identification claim.

== Content

- Embedded text
- Multiple pages
- Vector graphics

#pagebreak()

= Second page

#circle(radius: 12mm, fill: rgb("#f1dca7"), stroke: 0.8pt + rgb("#8a6522"))
