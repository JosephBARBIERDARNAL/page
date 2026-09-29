Each profile cell is the relative speedup of page over veraPDF for that profile (veraPDF runtime divided by page runtime); **higher is faster**. Values use the median of 10 measured runs with 2 warmup runs.

The corpus includes real world PDFs with many pages, images, an embedded font, structure elements, optional-content groups, a name tree, and embedded files. We're currently working on sharing the documents used to make the process fully reproducible.

| Document | Size (MiB) | Pages | PDF/A-1b | PDF/A-2b | PDF/UA-1 |
| --- | ---: | ---: | ---: | ---: | ---: |
| document 1 | 21.4 | 756 | 4.62× | 4.74× | 6.05× |
| document 2 | 6.5 | 51 | 8.22× | 8.36× | 8.00× |
| document 3 | 38.2 | 518 | 3.47× | 3.83× | 3.17× |
| document 4 | 9.6 | 332 | 4.45× | 4.74× | 4.13× |
| document 5 | 5.7 | 88 | 2.53× | 2.65× | 2.98× |
!!! info

       When details of which rule failed are not required, validation is expected to be much, much faster. Internally, this mode is called **lazy mode** and brings an **additional 2× to 10× speed improvement**. This is not represented in this benchmark to keep it simpler.
