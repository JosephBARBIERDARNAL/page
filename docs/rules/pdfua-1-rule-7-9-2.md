# PDF/UA-1 rule 7.9-2

Note structure elements must not share a non-empty `/ID` string.

The check reuses the bounded structure-tree traversal and applies to effective `Note` types after `/RoleMap` resolution. It tracks non-empty string IDs across the structure tree and reports every Note participating in a duplicate ID.

This matches veraPDF 1.30.2's `hasDuplicateNoteID == false` predicate; no intentional scope reduction or veraPDF discrepancy is known.
