Median validation timings are in milliseconds; each cell is `median ± IQR` over 10 measured runs with 2 warmups. The release CLI is measured end to end, including process startup.

The overview compares PDF/UA-1 with veraPDF across the full corpus; the profile matrix compares every implemented profile on the representative document and feature-heavy stress case.

The corpus includes the existing documents and a deterministic feature-heavy PDF covering structure, optional-content, name-tree, and embedded-file paths.

## Overview (PDF/UA-1)

| Document | Size (MiB) | page (ms) | fail-fast (ms) | veraPDF (ms) | page / veraPDF | fail-fast / veraPDF |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| document-feature-heavy.pdf | 4.0 | 81.5 ± 0.6 | 72.6 ± 0.3 | 1218.6 ± 22.8 | 14.96× | 16.78× |
| document1.pdf | 21.4 | 2419.2 ± 47.9 | 1166.0 ± 24.9 | 15007.6 ± 750.8 | 6.20× | 12.87× |
| document2.pdf | 6.5 | 326.6 ± 5.1 | 146.1 ± 1.6 | 2407.6 ± 85.0 | 7.37× | 16.48× |
| document3.pdf | 38.2 | 1242.5 ± 33.9 | 600.0 ± 2.9 | 3277.2 ± 108.1 | 2.64× | 5.46× |
| document4.pdf | 9.6 | 1478.6 ± 35.5 | 226.9 ± 1.1 | 5605.2 ± 164.4 | 3.79× | 24.70× |
| document5.pdf | 5.7 | 1673.8 ± 40.0 | 164.1 ± 1.1 | 3316.4 ± 130.2 | 1.98× | 20.21× |

## Profile matrix

Each cell is `page / fail-fast` speedup versus veraPDF; higher is faster.

| Document | 1b | 2b | 3b | ua1 |
| --- | ---: | ---: | ---: | ---: |
| document-feature-heavy.pdf | 16.13× / 17.77× | 15.93× / 17.75× | 15.53× / 17.32× | 14.96× / 16.78× |
| document2.pdf | 7.76× / 16.92× | 7.66× / 16.94× | 7.66× / 16.88× | 7.37× / 16.48× |

The feature-heavy case is intentionally non-compliant; it is included to make profile-demand differences measurable rather than to represent a normal publishing workload.
