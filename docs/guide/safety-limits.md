To limit resource use when processing untrusted PDFs, `page` enforces configurable bounds on input size, decoded data, parsed objects, reference depth, and selected inspection work.

| Option CLI                         |   Default | Purpose                                                                             |
| ---------------------------------- | --------: | ----------------------------------------------------------------------------------- |
| `--max-input-size`                 |   256 MiB | Maximum input file size.                                                            |
| `--max-decoded-stream-size`        |    32 MiB | Maximum decoded size of one stream.                                                 |
| `--max-total-decoded-content-size` |   256 MiB | Maximum total decoded page, Form, appearance, Pattern, and Type3 content.           |
| `--max-object-count`               | 1,000,000 | Maximum number of parsed indirect objects.                                          |
| `--max-reference-depth`            |       256 | Maximum reference-chain depth.                                                      |
| `--max-xref-revisions`             |     1,024 | Maximum number of incremental-update revisions read from the cross-reference chain. |
| `--max-table-span`                 |     1,024 | Maximum number of rows or columns covered by one table cell.                        |
| `--max-table-grid-rows`            |     1,024 | Maximum number of rows represented in an inspected table grid.                      |
| `--max-table-grid-columns`         |     1,024 | Maximum number of columns represented in an inspected table grid.                   |
| `--max-table-grid-cells`           | 1,000,000 | Maximum number of cells represented in an inspected table grid.                     |
| `--max-unicode-cmap-mappings`      | 1,000,000 | Maximum number of mappings expanded from one ToUnicode CMap.                        |

The defaults try to be convenient (they should be large enough for most files) while ensuring some level of default security. Obviously those are configurables, **with no limits** (see [this issue](https://github.com/JosephBARBIERDARNAL/page/issues/301)).

Disabling/changing safety limits is a security risk and must be done with control.
