# PDF/UA-1 rule 7.4.4-1

Each structure element may contain at most one direct child whose effective structure type is `/H`.

The check runs during the existing bounded depth-first structure-tree traversal, counts direct structure-element children after `/RoleMap` resolution, and intentionally treats `/H1`–`/H6` as different tags. This matches veraPDF 1.30.2's `kidsStandardTypes` predicate; no intentional scope reduction or veraPDF discrepancy is known.
