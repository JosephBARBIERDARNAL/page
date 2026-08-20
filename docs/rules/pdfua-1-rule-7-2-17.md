# PDF/UA-1 rule 7.2-17

An `LI` structure element must be directly contained in an `L` structure element.

The check reuses the bounded structure-tree traversal and compares each effective `LI` parent type after `/RoleMap` resolution with `L`. This matches veraPDF 1.30.2's `parentStandardType == 'L'` predicate; no intentional scope reduction or veraPDF discrepancy is known.
