#set document(
  title: [page PDF/A-1a content composition fixture],
  author: "page test suite",
  description: [A deterministic PDF/A-1a fixture with graphics and an image.],
  date: datetime(year: 2026, month: 1, day: 1),
)
#set page(paper: "a4", margin: 25mm)
#set text(font: "Libertinus Serif", lang: "en", size: 10pt)

#heading(level: 1)[Graphics and image content]

The page combines ordinary text, vector paths, a reusable box, and an embedded
image under one PDF/A-1a output intent.

#rect(width: 70mm, height: 25mm, fill: rgb("#d9e8f5"), stroke: 1pt + black)

#image("composition-image.svg", width: 40mm, alt: "A blue circle and green triangle")
