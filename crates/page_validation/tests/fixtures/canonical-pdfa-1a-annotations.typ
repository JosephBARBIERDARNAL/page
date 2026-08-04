#set document(
  title: [page PDF/A-1a annotation composition fixture],
  author: "page test suite",
  description: [A deterministic PDF/A-1a fixture with a valid link annotation.],
  date: datetime(year: 2026, month: 1, day: 1),
)
#set page(paper: "a4", margin: 25mm)
#set text(font: "Libertinus Serif", lang: "en", size: 10pt)

#heading(level: 1)[Annotation composition]

This paragraph contains a standards-permitted URI link annotation with a
visible text appearance.

#link("https://example.com")[Open the reference site]
