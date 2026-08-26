---
title: "Faillures"
---

Each report contains a list of failures:

```rust
use std::path::Path;
use page_validation::{SafetyLimits, validate_file};

let report = validate_file(Path::new("file.pdf"), &SafetyLimits::default())?;

for failure in &report.failures {
    println!("Rule: {}", failure.rule_id);
    println!("Category: {:?}", failure.category);
    println!("Message: {}", failure.message);
}
```

Failure categories distinguish conformance problems from parser or operational errors:

```rust
use page_validation::FailureCategory;

match failure.category {
    FailureCategory::Metadata | FailureCategory::Conformance => {
        // The PDF was parsed, but failed a validation rule.
    }
    FailureCategory::Parser => {
        // The PDF could not be parsed correctly.
    }
    FailureCategory::Operational => {
        // Validation failed because of I/O or another runtime issue.
    }
}
```

## Use the exit code

For command-line integrations or automated checks, the report can provide an appropriate process exit code:

```rust
std::process::exit(report.exit_code());
```
