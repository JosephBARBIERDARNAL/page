# `ValidationProfile`

**Enum**

A PDF/A or PDF/UA conformance level this crate can validate a document against.

A profile is either declared by a document's own XMP identification schema or selected explicitly by a caller through the optional `profile` argument accepted by [`validate_bytes`], [`validate_file`], and [`is_pdf_compliant`]. Not every profile in this enum is implemented yet; `Self::is_implemented` reports which ones a `ValidationReport`'s `checks_passed` can be trusted for, and `Self::implemented_check_count` reports how many rules currently back that result.

## Examples

```rs
use page_validation::ValidationProfile;

assert_eq!(ValidationProfile::PdfA1b.to_string(), "PDF/A-1b");
assert!(ValidationProfile::PdfA1b.is_implemented());
```
