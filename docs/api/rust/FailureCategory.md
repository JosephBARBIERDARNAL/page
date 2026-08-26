# `FailureCategory`

**Enum**

The kind of problem a `ValidationFailure` represents, separating operational and parsing concerns from PDF/A or PDF/UA conformance itself.

`Operational` covers input that could not be read or exceeded a configured `SafetyLimits` bound; `Parser` covers input the strict PDF parser rejected outright; `Metadata` covers XMP or document-information problems; `Conformance` covers every other rule violation. `ValidationReport::has_operational_failure` and `ValidationReport::exit_code` both key off whether any recorded failure is `Operational`.

## Examples

```rs
use page_validation::FailureCategory;

assert!(FailureCategory::Operational < FailureCategory::Conformance);
```
