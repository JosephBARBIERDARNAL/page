Internally, `page` has 2 validation modes: _exhaustive_ and _lazy_. Whether it should use one or the other only depends on what the user asks for and is **automatically handled**. The _lazy_ mode will return _*as early as possible*_ and avoid spending time checking rules if the user only cares about whether the PDF is compliant or not.

In practice, the _lazy_ mode will stop at the very first failed rule it finds, while the _exhaustive_ mode runs all individual rule checks before returning its report.

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
    Rules   : 3 failed rules / 104 total
    Profile : PDF/UA-1
    Time    : 0.017s

    [ISO 14289-1:2014:5:1]
      Metadata: XMP does not contain the PDF/UA Identification schema
      Possible cause: The PDF/UA identification metadata is absent, malformed, or inconsistent with PDF/UA-1.
      Suggested fix: Correct the PDF/UA identification properties in the exported XMP metadata.

    [ISO 14289-1:2014:5:2]
      Metadata: XMP declares PDF/UA part values ["2"], expected exactly one value 1
      Possible cause: The PDF/UA identification metadata is absent, malformed, or inconsistent with PDF/UA-1.
      Suggested fix: Correct the PDF/UA identification properties in the exported XMP metadata.
    ```

    This example shows two of the three failures. In an interactive terminal, the labels before the colons are bold.

In the CLI, if you don't specify either `--format details` or `--format json`, the output is reduced and often **much faster**. Performance improvements can vary a lot depending on the document, but you can easily expect something between 2x and 10x most of the time.

!!! note

    On a conformant document, _lazy_ and _exhaustive_ modes behave exactly the same.

If you use `page` from Rust, Python, or JavaScript, the _lazy_ mode is used when calling `is_pdf_compliant`-like functions. `validate_pdf`-like functions will use _exhaustive_ mode.
