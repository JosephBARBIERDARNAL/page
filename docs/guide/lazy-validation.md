Internally, `page` has 2 validation modes: _exhaustive_ and _lazy_. Whether it should use one or the other only depends on what the user asks for and is **automatically handled**. The _lazy_ mode will return _*as early as possible*_ and avoid spending time checking rules if the user only cares about whether the PDF is compliant or not.

In practice, the _lazy_ mode will stop at the very first failed rule it finds, while the _exhaustive_ mode runs all individual rule checks before returning its report.

For example:

=== "Lazy"

    ```console
    $ page document.pdf --profile ua1
    Result  : Non-conformant
    Profile : PDF/UA-1
    Time    : 0.005s
    ```

=== "Exhaustive"

    ```console
    $ page document.pdf --profile ua1 --format details
    Result  : Non-conformant
    Rules   : 1 failed rule / 104 total
    Profile : PDF/UA-1
    Time    : 0.017s

    [ISO 14289-1:2014:7.18.1:2] Conformance: annotation 0 on page 1 has neither a non-empty /Contents entry nor a non-empty /Alt entry in its enclosing structure element
      Location: object 41 0 R (page 1)
    ```

    In an interactive terminal, the labels before the colons are bold. Detailed output previews up to three locations per finding; `--locations all` shows every known location.

In the CLI, summary output uses lazy validation. This is the default, and you can also select it explicitly with `--format summary`. Detailed and JSON output use exhaustive validation, including JSON selected automatically by `--output report.json`. Performance improvements from lazy validation vary by document; see the [benchmark](../benchmark.md).

!!! note

    On a conformant document, both modes must check every applicable implemented rule to establish compliance. Exhaustive validation additionally returns the full report.

If you use `page` from Rust, Python, or JavaScript, the _lazy_ mode is used when calling `is_pdf_compliant`-like functions. `validate_pdf`-like functions will use _exhaustive_ mode.
