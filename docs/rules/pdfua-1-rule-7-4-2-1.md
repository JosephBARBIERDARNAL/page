# PDF/UA-1 rule 7.4.2-1

Numbered heading elements must start with H1. Repeated levels are allowed, and a heading may move to a shallower level freely, but a move to a deeper level may advance only one level from the previous numbered heading.

The check runs during the existing bounded depth-first structure-tree traversal and uses effective heading types after `/RoleMap` resolution. This matches veraPDF 1.30.2's `hasCorrectNestingLevel` predicate; no intentional scope reduction or veraPDF discrepancy is known.
