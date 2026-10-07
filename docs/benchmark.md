Each profile cell is the relative speedup of page over veraPDF for that profile (veraPDF runtime divided by page runtime); **higher is faster**. Values use the median of 10 measured runs with 2 warmup runs.

The benchmark uses publicly available documents, which you can find [here](https://github.com/JosephBARBIERDARNAL/page-fixtures).

| Document | Size (MiB) | Pages | PDF/A-1b | PDF/A-2b | PDF/UA-1 |
| --- | ---: | ---: | ---: | ---: | ---: |
| gao-2024 | 10.5 | 153 | 5.6× | 6.2× | 5.7× |
| health-of-canadians-2025 | 1.4 | 84 | 12.4× | 12.0× | 9.6× |
| standards-for-development-world-bank | 10.7 | 408 | 6.0× | 6.1× | 5.9× |
| surveillance-drug-risk-2017 | 1.3 | 83 | 9.6× | 10.3× | 8.6× |
| world-health-statistics-2025 | 5.7 | 88 | 2.5× | 2.7× | 2.5× |
!!! info

       When details of which rule failed are not required, validation is expected to be much, much faster. Internally, this mode is called **lazy mode** and brings an **additional 2× to 10× speed improvement**. This is not represented in this benchmark to keep it simpler.
