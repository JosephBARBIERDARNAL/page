# `ValidationCounts`

**Struct**

A tally of how many implemented checks ran against a document and how many of those passed or failed.

`total` is always `passed + failed`; it does not count checks for rules that are not yet implemented for the report's `ValidationProfile`, so a `checks_passed` report can still be missing coverage that `ValidationProfile::implemented_check_count` and the corpus/differential tooling track separately.

## Examples

```rs
use page_validation::ValidationCounts;

let counts = ValidationCounts {
    total: 5,
    passed: 5,
    failed: 0,
};
assert_eq!(counts.total, counts.passed + counts.failed);
```
