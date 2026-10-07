Internally, `page` has 2 validation modes: _exhaustive_ and _lazy_. Whether it should use one or the other only depends on what user ask for and is **automatically handled**. The _lazy_ mode will return **as early as possible** and avoid spending time checking for rules if user only care about whether the PDF is compliant or not.

In practice, the _lazy_ mode will stop at the very first failed rule it founds, while the _exhaustive_ mode runs all individual rule checks before returning its report.

For example:

=== "Lazy"

      ```console
      $ page document.pdf --profile ua1
      Result  : Non-conformant
      Profile : PDF/UA-1
      Time    : 0.052s
      ```

=== "Exhaustive"

      ```console
      $ page document.pdf --profile ua1 --format details
      Result  : Non-conformant
      Rules   : 11 failed rules / 134 total
      Checks  : 24 failed checks
      Profile : PDF/UA-1
      Time    : 0.147s

      [PDFUA1-ALT-TEXT-LANGUAGE-001] [.........]
      [PDFUA1-CONTENT-TAGGING-001] [.........]
      [PDFUA1-FIGURE-ALTERNATIVE-TEXT-001] [.........]
      [PDFUA1-HEADING-NESTING-001] [.........]
      [PDFUA1-ID-PART-001] [.........]
      [PDFUA1-ID-SCHEMA-001] [.........]
      [PDFUA1-LINK-CONTENTS-001] [.........]
      [PDFUA1-METADATA-TITLE-001] [.........]
      [PDFUA1-SPAN-ACTUAL-TEXT-LANGUAGE-001] [.........]
      [PDFUA1-TABLE-COLUMN-ROWSPAN-001] [.........]
      [PDFUA1-TABLE-HEADERS-SCOPE-001] [.........]
      [PDFUA1-TABLE-ROW-COLUMNSPAN-001] [.........]
      [PDFUA1-TAGGED-DOCUMENT-001] [.........]
      [PDFUA1-TEXT-LANGUAGE-001] [.........]
      ```

In the CLI, if you don't specify either `--format details` or `--format json`, the output is reduced, and often **much faster**. Performance improvements can vary a lot depending on the document, but you can easily expect something between 2 to 10x most of the time.

!!! note

      On a conformant document, _lazy_ and _exhaustive_ modes behave exactly the same.

If you use `page` from Rust, Python or JavaScript, the _lazy_ mode is used when calling `is_pdf_compliant`-like functions. `validate_pdf`-like functions will use _exhaustive_ mode.
