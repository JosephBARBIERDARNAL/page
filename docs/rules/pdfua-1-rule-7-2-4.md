# PDF/UA-1 rule 7.2-4

`page` checks each resolved `TR` structure element's direct parent standard type and accepts only `Table`, `THead`, `TBody`, or `TFoot`, including parent types reached through RoleMap.

This matches veraPDF 1.30.2 for the focused direct-parent cases. The check intentionally follows veraPDF's `parentStandardType` scope: it validates the structure-tree parent relationship and does not additionally validate the row's children, which are covered by separate PDF/UA-1 table rules.
