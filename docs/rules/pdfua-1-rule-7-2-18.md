# PDF/UA-1 rule 7.2-18

An `LBody` structure element must be directly contained in an `LI` structure element.

The check reuses the bounded structure-tree traversal and compares each effective `LBody` parent type after `/RoleMap` resolution with `LI`. This matches veraPDF 1.30.2's `parentStandardType == 'LI'` predicate; no intentional scope reduction or veraPDF discrepancy is known.
