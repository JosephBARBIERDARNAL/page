# `DocumentMetadata`

**Struct**

The document information dictionary (PDF32000 §14.3.3), captured as a flat map from entry name to its decoded text value.

`values` holds whichever of `Title`, `Author`, `Subject`, `Keywords`, `Creator`, `Producer`, `CreationDate`, `ModDate`, and `Trapped` are present in the trailer's `/Info` dictionary; an entry absent from the source document is simply absent from the map, and a document without an `/Info` dictionary produces an empty [`Self::default`]. This is the legacy sibling of the XMP-derived [`XmpMetadata`], compared against it by the `PDFA1B-INFO-*` consistency rules.

## Examples

```
use page_validation::DocumentMetadata;

let info = DocumentMetadata::default();
assert!(info.values.is_empty());
```
