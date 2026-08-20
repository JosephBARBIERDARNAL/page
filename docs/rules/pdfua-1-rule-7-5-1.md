# PDF/UA-1 rule 7.5-1

When a table's cell headers cannot be determined from `/Headers` and `/ID`, `TH` cells must carry an explicit Table `/Scope` attribute.

The implementation reuses the bounded structure-tree table grid and mirrors veraPDF 1.30.2's PDF/UA-1 algorithm: regular tables are checked using explicit Table `/Scope` values, then headers are inferred above or to the left of each non-top-left `TD`. Undefined `/Headers` values remain outside this rule and belong to 7.5-2.

Focused fixtures are differentially tested against veraPDF 1.30.2. No intentional discrepancy is known for this rule.
