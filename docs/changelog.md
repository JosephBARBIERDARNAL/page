## Dev

- Perf: Consolidate raw syntax and stream scans [#328](https://github.com/JosephBARBIERDARNAL/page/issues/328)
- Perf: Cache font usages, shown bytes, CIDs, and embedded font streams [#327](https://github.com/JosephBARBIERDARNAL/page/issues/327)
- Perf: Defer whole-document font summaries on the fast validation path [#326](https://github.com/JosephBARBIERDARNAL/page/issues/326)
- Doc: Add page dedicated to safety limits [#320](https://github.com/JosephBARBIERDARNAL/page/issues/320)
- Perf: Expand the benchmark with profile comparisons and a feature-heavy fixture
- Perf: Make document feature inspection profile-demand driven [#325](https://github.com/JosephBARBIERDARNAL/page/issues/325)
- Perf: Inferred-profile summary mode now uses fail-fast validation [#324](https://github.com/JosephBARBIERDARNAL/page/issues/324)

## 0.7.0

- Fix: Invalid error message made for the CLI is used by all consumers [#312](https://github.com/JosephBARBIERDARNAL/page/issues/312)
- Fix: ToUnicode CMap ranges lack a cumulative expansion limit [#318](https://github.com/JosephBARBIERDARNAL/page/issues/318)
- Fix: Tagged-table spans can cause unbounded grid allocations [#317](https://github.com/JosephBARBIERDARNAL/page/issues/317)
- Fix: font stream decode fallback bypasses max_decoded_stream_size [#316](https://github.com/JosephBARBIERDARNAL/page/issues/316)
- Fix: encrypted PDFs can bypass the configured object-count limit [#315](https://github.com/JosephBARBIERDARNAL/page/issues/315)
- Doc: make a better landing page [#282](https://github.com/JosephBARBIERDARNAL/page/issues/282)
- Doc: add changelog page [#274](https://github.com/JosephBARBIERDARNAL/page/issues/274)
