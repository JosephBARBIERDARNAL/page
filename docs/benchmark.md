Each profile cell is the relative speedup of page over veraPDF for that profile (veraPDF runtime divided by page runtime); **higher is faster**. Values use the median of 10 measured runs with 2 warmup runs.

The corpus includes real world documents and a deterministic feature-heavy PDF with multiple pages, images, an embedded font, structure elements, optional-content groups, a name tree, and embedded files. We're currently working on sharing the documents used to make the process fully reproducible.

| Document | Size (MiB) | Pages | PDF/A-1b | PDF/A-2b | PDF/UA-1 |
| --- | ---: | ---: | ---: | ---: | ---: |
| feature-heavy | 4.0 | 10 | 16.96× | 17.22× | 16.63× |
| document 1 | 21.4 | 756 | 4.53× | 4.81× | 6.34× |
| document 2 | 6.5 | 51 | 8.44× | 8.21× | 8.62× |
| document 3 | 38.2 | 518 | 3.12× | 3.21× | 2.53× |
| document 4 | 9.6 | 332 | 4.05× | 4.35× | 3.71× |
| document 5 | 5.7 | 88 | 2.01× | 2.09× | 1.92× |
!!! info

       When details of which rule failed are not required, validation is expected to be much, much faster (an additional 2× and 10× improvement); this is not represented in this benchmark to keep it simpler.
