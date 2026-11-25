# Performance baseline

Source: successful branch-push CI run [45529398110](https://github.com/MykytaStel/hexscope/actions/runs/45529398110), PR #208, Linux runner, gzip -9c. These are compressed transfer bytes, not memory usage.

| Module | Baseline bytes | CI limit |
| --- | ---: | ---: |
| media | 176422 | 178000 |
| pdf | 227779 | 229000 |
| docs | 238387 | 240000 |
| audio | 166766 | 168000 |
| raster | 214454 | 216000 |

Run `pnpm build` then `pnpm perf:size --check` (or `pnpm perf:size --json`) to collect comparable candidate evidence. Vite's gzip estimate is not the CI measurement. No budget increases in this slice.

Memory changes are structural, not measured RSS claims: released presentation allocations can be reused by the WASM allocator, but linear memory does not shrink; the sparse movie view still reserves the full logical file length. Measure physical-device peak memory and repeated open/clean/back cycles before claiming improvements in RSS or larger supported files. Documents not prefetched must be opened online once before offline use.
