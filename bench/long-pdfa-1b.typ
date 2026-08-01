#set document(title: "Long PDF/A-1b benchmark document", author: "tag")

= Long PDF/A-1b benchmark document

This document is intentionally long and content-heavy so that benchmark runs include meaningful parsing and validation work in addition to process startup.

#for section in range(1, 81) [
  = Section #section
  #lorem(220)
  #lorem(220)
  #if section < 80 [#pagebreak()]
]
