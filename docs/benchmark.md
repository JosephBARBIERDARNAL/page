Mean validation speedups are based on timings measured in milliseconds and normalized per document to veraPDF = 1.0×. Higher values are faster. Each value uses 10 runs (with 2 warmup runs) per validator and document.

| Document      | veraPDF | page fail-fast | page |
| ------------- | ------: | -------------: | ---: |
| document1.pdf |    1.0× |          17.4× | 7.0× |
| document2.pdf |    1.0× |          19.2× | 7.8× |
| document3.pdf |    1.0× |           6.4× | 2.8× |
| document4.pdf |    1.0× |          31.4× | 3.9× |
| document5.pdf |    1.0× |          28.5× | 1.9× |

!!! note

      The fail fast mode of `page` (used automatically when possible) allows to get much faster results, but does not give details about which specific rules failed.
