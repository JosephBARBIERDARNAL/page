// Canonical PDF/A-1b baseline. Keep this source deterministic; the fixed
// metadata date and compile timestamp are part of the fixture contract.
#set document(
  title: [page canonical PDF/A-1b fixture],
  author: "page test suite",
  description: [A deterministic PDF/A-1b conformance baseline for page and veraPDF.],
  keywords: ("PDF/A-1b", "veraPDF", "Typst"),
  date: datetime(year: 2026, month: 1, day: 1),
)
#set page(paper: "a4", margin: 25mm)
#set text(font: "Libertinus Serif", lang: "en", size: 10pt)
#set heading(numbering: "1.")

#title()

This canonical document contains embedded text, document metadata, vector
graphics, and a colour-managed output intent.

== Content

The baseline exercises the shared PDF/A-1b content, font, colour, and graphics
checks.

#rect(
  width: 100%,
  height: 18mm,
  fill: rgb("#d9e8f5"),
  stroke: 0.8pt + rgb("#285f8f"),
  radius: 2pt,
)

#table(
  columns: (1fr, 2fr),
  inset: 5pt,
  stroke: 0.5pt + rgb("#285f8f"),
  table.header([Property], [Expected value]),
  [PDF version], [1.4],
  [Conformance], [PDF/A-1b],
  [Reference validator], [veraPDF 1.30.2],
)
