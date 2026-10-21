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
    Rules   : 11 failed rules / 134 total
    Checks  : 24 failed checks
    Profile : PDF/UA-1
    Time    : 0.147s

    [ISO 14289-1:2014:7.2:22] [.........]
    [ISO 14289-1:2014:7.1:3] [.........]
    [ISO 14289-1:2014:7.3:1] [.........]
    [ISO 14289-1:2014:7.4.2:1] [.........]
    [ISO 14289-1:2014:5:2] [.........]
    [ISO 14289-1:2014:5:1] [.........]
    [ISO 14289-1:2014:7.18.5:2] [.........]
    [ISO 14289-1:2014:7.1:9] [.........]
    [ISO 14289-1:2014:7.2:30] [.........]
    [ISO 14289-1:2014:7.2:41] [.........]
    [ISO 14289-1:2014:7.5:1] [.........]
    [ISO 14289-1:2014:7.2:42] [.........]
    [ISO 14289-1:2014:6.2:1] [.........]
    [ISO 14289-1:2014:7.2:34] [.........]
    ```

In the CLI, if you don't specify either `--format details` or `--format json`, the output is reduced and often **much faster**. Performance improvements can vary a lot depending on the document, but you can easily expect something between 2x and 10x most of the time.

!!! note

    On a conformant document, _lazy_ and _exhaustive_ modes behave exactly the same.

If you use `page` from Rust, Python, or JavaScript, the _lazy_ mode is used when calling `is_pdf_compliant`-like functions. `validate_pdf`-like functions will use _exhaustive_ mode.
