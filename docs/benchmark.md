Mean validation speedups are based on timings measured in milliseconds and normalized per document to veraPDF = 1.0×. Higher values are faster. Each value uses 10 runs (with 2 warmup runs) per validator and document.

The 5 documents have varying size, content and method of creation. We're working on making the benchmark fully reproducible and share the documents used.

| Document      | veraPDF | page | page fail-fast |
| ------------- | ------: | ---: | -------------: |
| document1.pdf |    1.0× | 6.2× |          13.7× |
| document2.pdf |    1.0× | 7.8× |          17.5× |
| document3.pdf |    1.0× | 2.6× |           5.4× |
| document4.pdf |    1.0× | 3.8× |          23.6× |
| document5.pdf |    1.0× | 1.9× |          19.5× |

!!! note

      The fail fast mode of `page` (used automatically when possible) allows to get much faster results, but does not give details about which specific rules failed.
