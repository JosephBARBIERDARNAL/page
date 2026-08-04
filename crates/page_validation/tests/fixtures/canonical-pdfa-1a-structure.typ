#set document(
  title: [page PDF/A-1a structure composition fixture],
  author: "page test suite",
  description: [A deterministic PDF/A-1a fixture with nested logical structure.],
  date: datetime(year: 2026, month: 1, day: 1),
)
#set page(paper: "a4", margin: 25mm)
#set text(font: "Libertinus Serif", lang: "en", size: 10pt)
#set heading(numbering: "1.")

#heading(level: 1)[Structured section]
The section contains nested headings and paragraphs.

== Nested subsection

The structure tree contains a parent section, a child heading, and ordinary
paragraph content with a document language.

=== Nested subsection detail

This text exercises indirect structure elements and their parent links.
