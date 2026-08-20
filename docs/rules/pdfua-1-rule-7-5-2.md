# PDF/UA-1 rule 7.5-2

When a table cell's `/Headers` attribute references an undefined header ID and the table's headers cannot be determined algorithmically, the table's `TH` cells must carry an explicit Table `/Scope` attribute.

The implementation reuses the bounded structure-tree table grid and header-scope inference used by rule 7.5-1. It reports only undefined `/Headers` references; missing `/Headers` attributes remain covered by rule 7.5-1.

Focused fixtures are differentially tested against veraPDF 1.30.2. No intentional discrepancy is known for this rule.
