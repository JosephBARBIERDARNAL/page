# PDF/UA-1 rule 7.4.4-2

Documents must use either the weak heading structure (`/H`) or the strong heading structure (`/H1`–`/H6`), but not both.

The check runs after the existing bounded depth-first structure-tree traversal, resolves effective structure types through `/RoleMap`, and reports every `/H` element when any numbered heading is present. This mirrors veraPDF 1.30.2's deferred `usesHn` predicate, including headings encountered before the numbered heading. The companion 7.4.4-3 check for `/H1`–`/H6` elements in the presence of `/H` remains outside this task's scope, so mixed-heading documents can still show that separate veraPDF coverage gap.
