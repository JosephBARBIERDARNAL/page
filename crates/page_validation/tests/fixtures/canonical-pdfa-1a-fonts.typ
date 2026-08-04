#set document(
  title: [page PDF/A-1a font composition fixture],
  author: "page test suite",
  description: [A deterministic PDF/A-1a fixture with multiple embedded fonts.],
  date: datetime(year: 2026, month: 1, day: 1),
)
#set page(paper: "a4", margin: 25mm)
#set text(lang: "en", size: 11pt)

#heading(level: 1)[Embedded font composition]

#text(font: "Libertinus Serif")[Serif text with a Unicode mapping.]

#parbreak()

#text(font: "New Computer Modern")[Modern text with a separate embedded font.]

#parbreak()

#text(font: "DejaVu Sans Mono")[Monospaced text with an independent encoding.]
