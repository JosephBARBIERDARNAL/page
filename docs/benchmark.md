Each profile cell is the relative speedup of page over veraPDF for that profile (veraPDF runtime divided by page runtime); **higher is faster**. Values use the median of 10 measured runs with 2 warmup runs.

The benchmark uses publicly available documents, you can find them [here](https://github.com/JosephBARBIERDARNAL/page-fixtures).

=== "Exhaustive"

    Each cell is the relative speedup of page over veraPDF for that profile (veraPDF runtime divided by page runtime); **higher is faster**.

    | Document | Size (MiB) | Pages | PDF/A-1b | PDF/A-2b | PDF/UA-1 |
    | --- | ---: | ---: | ---: | ---: | ---: |
    | gao-2024 | 10.5 | 153 | 5.3× | 5.7× | 5.1× |
    | health-of-canadians-2025 | 1.4 | 84 | 10.2× | 10.8× | 9.0× |
    | standards-for-development-world-bank | 10.7 | 408 | 5.6× | 6.0× | 5.8× |
    | surveillance-drug-risk-2017 | 1.3 | 83 | 8.7× | 9.1× | 8.0× |
    | world-health-statistics-2025 | 5.7 | 88 | 2.4× | 2.5× | 2.4× |

=== "Lazy"

    Each cell is the relative speedup of page over veraPDF for that profile (veraPDF runtime divided by page runtime); **higher is faster**.

    | Document | Size (MiB) | Pages | PDF/A-1b | PDF/A-2b | PDF/UA-1 |
    | --- | ---: | ---: | ---: | ---: | ---: |
    | gao-2024 | 10.5 | 153 | 23.0× | 24.8× | 22.3× |
    | health-of-canadians-2025 | 1.4 | 84 | 45.0× | 47.4× | 39.9× |
    | standards-for-development-world-bank | 10.7 | 408 | 25.2× | 27.4× | 26.9× |
    | surveillance-drug-risk-2017 | 1.3 | 83 | 54.4× | 56.5× | 50.2× |
    | world-health-statistics-2025 | 5.7 | 88 | 20.5× | 21.6× | 20.3× |

## Benchmark machine

- OS: macOS 26.6.2
- Model: MacBook Pro
- Processor: Apple M1 Pro (arm64)
- Memory: 32 GiB RAM
- CPU cores: 10 (8 Performance and 2 Efficiency)
