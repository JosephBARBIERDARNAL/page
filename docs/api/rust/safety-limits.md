---
title: "Safety limits"
---

The goal of the safety limits protect the validator from excessively large or complex inputs. Defaults are the following and should be sufficient for most cases:

```rust
use page_validation::SafetyLimits;

let limits = SafetyLimits {
    max_input_size: 100 * 1024 * 1024,                 // 100 MiB
    max_decoded_stream_size: 32 * 1024 * 1024,         // 32 MiB
    max_total_decoded_content_size: 100 * 1024 * 1024, // 100 MiB
    max_object_count: 500_000,                         // 500,000 objects
    max_reference_depth: 256,                          // 256 levels
};
```

`max_decoded_stream_size` bounds one decoded stream and `max_total_decoded_content_size` bounds the total decoded page, Form, appearance, Pattern, and Type3 content inspected for one document.
