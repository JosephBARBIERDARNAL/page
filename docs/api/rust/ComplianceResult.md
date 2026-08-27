# `ComplianceResult`

**Struct**

The selected profile and compliance outcome returned by [`is_pdf_compliant`].

`profile` is either the explicitly requested profile or the one inferred
from the document's XMP metadata. `is_compliant` is `false` as soon as the
validator finds the first failing rule.
