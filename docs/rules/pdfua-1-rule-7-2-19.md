# PDF/UA-1 rule 7.2-19

An `L` structure element may contain only `L`, `LI`, and `Caption` structure elements.

The check reuses the bounded direct-structure-child traversal, resolves child types through `/RoleMap`, and ignores marked-content and integer kids like veraPDF's `kidsStandardTypes` predicate. This matches veraPDF 1.30.2; no intentional scope reduction or veraPDF discrepancy is known.
