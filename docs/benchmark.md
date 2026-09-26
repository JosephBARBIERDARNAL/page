Each profile cell is the relative speedup of page over veraPDF for that profile (veraPDF runtime divided by page runtime); **higher is faster**. Values use the median of 10 measured runs with 2 warmup runs.

The corpus includes real world documents and a deterministic feature-heavy PDF with multiple pages, images, an embedded font, structure elements, optional-content groups, a name tree, and embedded files. We're currently working on sharing the documents used to make the process fully reproducible.

| Document | Size (MiB) | Pages | PDF/A-1b | PDF/A-2b | PDF/UA-1 |
| --- | ---: | ---: | ---: | ---: | ---: |
| feature-heavy | 4.0 | 10 | 14.99× | 14.58× | 14.18× |
| document 1 | 21.4 | 756 | 4.38× | 5.95× | 6.10× |
| document 2 | 6.5 | 51 | 8.25× | 7.78× | 7.37× |
| document 3 | 38.2 | 518 | 3.18× | 3.37× | 2.78× |
| document 4 | 9.6 | 332 | 4.39× | 4.53× | 3.97× |
| document 5 | 5.7 | 88 | 2.43× | 2.55× | 2.34× |
!!! info

       When details of which rule failed are not required, validation is expected to be much, much faster (an additional 2× and 10× improvement); this is not represented in this benchmark to keep it simpler.
