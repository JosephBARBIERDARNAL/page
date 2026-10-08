Each profile cell is the relative speedup of page over veraPDF for that profile (veraPDF runtime divided by page runtime); **higher is faster**. Values use the median of 10 measured runs with 2 warmup runs. The benchmark uses publicly available documents, you can find them [here](https://github.com/JosephBARBIERDARNAL/page-fixtures).

This benchmark compares _exhaustive_ and _lazy_ validation modes of `page` against veraPDF. Learn more about the difference between those modes [here](guide/lazy-validation.md).

=== "Exhaustive"

    | Document | Size (MiB) | Pages | PDF/A-1b | PDF/A-2b | PDF/UA-1 |
    | --- | ---: | ---: | ---: | ---: | ---: |
    | gao-2024 | 10.5 | 153 | 5.2× | 5.5× | 5.0× |
    | health-of-canadians-2025 | 1.4 | 84 | 10.0× | 10.5× | 8.8× |
    | standards-for-development-world-bank | 10.7 | 408 | 5.6× | 6.1× | 5.7× |
    | surveillance-drug-risk-2017 | 1.3 | 83 | 8.7× | 9.0× | 8.0× |
    | world-health-statistics-2025 | 5.7 | 88 | 2.4× | 2.5× | 2.3× |

=== "Lazy"

    | Document | Size (MiB) | Pages | PDF/A-1b | PDF/A-2b | PDF/UA-1 |
    | --- | ---: | ---: | ---: | ---: | ---: |
    | gao-2024 | 10.5 | 153 | 22.3× | 23.9× | 22.0× |
    | health-of-canadians-2025 | 1.4 | 84 | 44.3× | 46.4× | 38.9× |
    | standards-for-development-world-bank | 10.7 | 408 | 25.3× | 27.4× | 26.1× |
    | surveillance-drug-risk-2017 | 1.3 | 83 | 55.0× | 56.6× | 49.6× |
    | world-health-statistics-2025 | 5.7 | 88 | 20.4× | 21.4× | 19.8× |

Machine used for the benchmark: macOS (26.6.2) with 10 (8 Performance and 2 Efficiency) CPU cores and 32 GiB of RAM.
