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

## Local candidate evidence, PR #209

Reported by the maintainer after passing tests on macOS, using `pnpm perf:size --check`:

| Module | gzip bytes | Headroom |
| --- | ---: | ---: |
| media | 176174 | 1826 |
| pdf | 228317 | 683 |
| docs | 238709 | 1291 |
| audio | 166611 | 1389 |
| raster | 213401 | 2599 |

Do not treat cross-platform differences as measured binary-size savings. PDF has the smallest margin. The next compact-range slice is TypeScript-only; retest the web bundle and existing gzip gates.
