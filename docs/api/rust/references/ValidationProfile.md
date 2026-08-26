# `ValidationProfile`

**Enum**

A PDF/A or PDF/UA conformance level this crate can validate a document against.

A profile is either declared by a document's own XMP identification schema (see [`validate_bytes`] and [`validate_file`]) or selected explicitly by a caller (see [`validate_bytes_with_profile`] and [`validate_file_with_profile`]). Not every profile in this enum is implemented yet; [`Self::is_implemented`] reports which ones a [`ValidationReport`](crate::ValidationReport)'s `checks_passed` can be trusted for, and [`Self::implemented_check_count`] reports how many rules currently back that result.

## Examples

```
use page_validation::ValidationProfile;

assert_eq!(ValidationProfile::PdfA1b.to_string(), "PDF/A-1b");
assert!(ValidationProfile::PdfA1b.is_implemented());
```
