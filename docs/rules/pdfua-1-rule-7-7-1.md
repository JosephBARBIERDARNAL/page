# PDF/UA-1 rule 7.7-1

Formula structure elements must provide either a non-empty `/Alt` string or an `/ActualText` string.

The check reuses the bounded structure-tree traversal, applies to effective `Formula` types after `/RoleMap` resolution, and matches veraPDF 1.30.2's predicate `(Alt != null && Alt != '') || ActualText != null`.

As with veraPDF's rule, this implementation checks Formula structure elements and does not infer whether untagged page content is mathematical notation.
