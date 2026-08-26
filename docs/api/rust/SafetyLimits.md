# `SafetyLimits`

**Struct**

Configurable bounds that keep PDF parsing and inspection resource use predictable regardless of what an untrusted input contains.

Each field caps a distinct resource: the raw input size, a single decoded stream, the sum of all decoded content streams, the number of indirect objects, the depth of a chased reference chain, and the number of incremental-update revisions read from the cross-reference chain. Exceeding any of these bounds during validation produces a `PdfError` variant instead of letting parsing or inspection consume unbounded memory or CPU. `Self::default` uses this type's `DEFAULT_*` associated constants.

## Examples

```rs
use page_validation::SafetyLimits;

let limits = SafetyLimits {
    max_input_size: 1024,
    ..SafetyLimits::default()
};
assert_eq!(limits.max_input_size, 1024);
assert_eq!(limits.max_object_count, SafetyLimits::DEFAULT_MAX_OBJECT_COUNT);
```
