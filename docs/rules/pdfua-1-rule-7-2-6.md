# PDF/UA-1 rule 7.2-6

TBody structure elements must be direct children of a Table structure element.

The check reuses the bounded structure-tree traversal and compares each resolved TBody parent type after /RoleMap resolution with Table. This matches veraPDF 1.30.2's `parentStandardType == 'Table'` predicate; it intentionally checks only the direct parent relationship, while TBody child validity remains covered by rule 7.2-37.
