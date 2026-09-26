Each profile cell is the relative speedup of page over veraPDF for that profile (veraPDF runtime divided by page runtime); **higher is faster**. Values use the median of 1 measured runs with 1 warmup runs.

The corpus includes the existing documents and a deterministic feature-heavy PDF with multiple pages, images, an embedded font, structure elements, optional-content groups, a name tree, and embedded files.

| Document | Size (MiB) | Pages | PDF/A-1b | PDF/A-2b | PDF/UA-1 |
| --- | ---: | ---: | ---: | ---: | ---: |
| feature-heavy | 4.0 | 10 | 15.40× | 15.81× | 15.53× |
| document 1 | 21.4 | 756 | 4.28× | 4.36× | 5.82× |
| document 2 | 6.5 | 51 | 7.24× | 7.68× | 7.34× |
| document 3 | 38.2 | 518 | 2.83× | 3.15× | 2.59× |
| document 4 | 9.6 | 332 | 4.21× | 4.20× | 3.69× |
| document 5 | 5.7 | 88 | 2.01× | 2.02× | 1.92× |
!!! info

       When details of which rule failed are not required, validation is expected to be much, much faster (between 2× and 10×); this is not represented in this benchmark to keep it simpler.
