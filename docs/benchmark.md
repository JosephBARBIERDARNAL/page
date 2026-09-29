Each profile cell is the relative speedup of page over veraPDF for that profile (veraPDF runtime divided by page runtime); **higher is faster**. Values use the median of 10 measured runs with 2 warmup runs.

The benchmark uses publicly available documents, you can find them [here](https://github.com/JosephBARBIERDARNAL/page-fixtures).

| Document | Size (MiB) | Pages | PDF/A-1b | PDF/A-2b | PDF/UA-1 |
| --- | ---: | ---: | ---: | ---: | ---: |
| gao-2024 | 10.5 | 153 | 5.20× | 5.66× | 5.13× |
| health-of-canadians-2025 | 1.4 | 84 | 10.71× | 11.56× | 9.20× |
| standards-for-development-world-bank | 10.7 | 408 | 5.75× | 6.01× | 5.85× |
| surveillance-drug-risk-2017 | 1.3 | 83 | 8.79× | 9.15× | 7.98× |
| world-health-statistics-2025 | 5.7 | 88 | 2.44× | 2.61× | 2.38× |
!!! info

       When details of which rule failed are not required, validation is expected to be much, much faster. Internally, this mode is called **lazy mode** and brings an **additional 2× to 10× speed improvement**. This is not represented in this benchmark to keep it simpler.
