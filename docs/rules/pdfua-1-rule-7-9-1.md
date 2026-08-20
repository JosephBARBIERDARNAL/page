# PDF/UA-1 rule 7.9-1

Note structure elements must provide a non-empty `/ID` string.

The check reuses the bounded structure-tree traversal, applies to effective `Note` types after `/RoleMap` resolution, and matches veraPDF 1.30.2's predicate `noteID != null && noteID != ''`.

No intentional scope reduction or veraPDF discrepancy is known for this rule.
