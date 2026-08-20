# PDF/UA-1 rule 7.3-1

Figure structure elements must provide either a non-empty `/Alt` string or an `/ActualText` string.

The check runs during the existing bounded structure-tree traversal and applies to effective `Figure` types after RoleMap resolution, matching veraPDF 1.30.2's predicate `(Alt != null && Alt != '') || ActualText != null`.

No intentional scope reduction or veraPDF discrepancy is known for this rule.
