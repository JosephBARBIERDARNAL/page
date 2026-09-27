Each profile cell is the relative speedup of page over veraPDF for that profile (veraPDF runtime divided by page runtime); **higher is faster**. Values use the median of 10 measured runs with 2 warmup runs.

The corpus includes real world documents and a deterministic feature-heavy PDF with multiple pages, images, an embedded font, structure elements, optional-content groups, a name tree, and embedded files. We're currently working on sharing the documents used to make the process fully reproducible.

| Document | Size (MiB) | Pages | PDF/A-1b | PDF/A-2b | PDF/UA-1 |
| --- | ---: | ---: | ---: | ---: | ---: |
| feature-heavy | 4.0 | 10 | 28.54× | 28.42× | 25.69× |
| document 1 | 21.4 | 756 | 4.91× | 4.98× | 6.55× |
| document 2 | 6.5 | 51 | 9.89× | 9.62× | 9.03× |
| document 3 | 38.2 | 518 | 4.33× | 4.72× | 3.82× |
| document 4 | 9.6 | 332 | 5.17× | 5.96× | 5.54× |
| document 5 | 5.7 | 88 | 3.36× | 2.73× | 2.46× |
!!! info

       When details of which rule failed are not required, validation is expected to be much, much faster (an additional 2× to 10× improvement); this is not represented in this benchmark to keep it simpler.
