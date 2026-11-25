# Performance evidence suite

Separate correctness tests from performance observations. Existing Criterion benchmarks, core large-file budgets, E2E and gzip gates remain unchanged.

## Collect locally

```sh
pnpm build
pnpm perf:test
pnpm --filter web exec playwright install chromium
HEXSCOPE_PERF_REPEATS=5 HEXSCOPE_PERF_OUTPUT=.perf/base-wasm.json pnpm perf:wasm
HEXSCOPE_PERF_REPEATS=3 HEXSCOPE_PERF_OUTPUT=.perf/base-desktop.json pnpm perf:browser
HEXSCOPE_PERF_PROFILE=phone HEXSCOPE_PERF_REPEATS=3 HEXSCOPE_PERF_OUTPUT=.perf/base-phone.json pnpm perf:browser
```

Run on the candidate with the same machine, Node/browser, repeat count and profile. Save to a different output path. Reports are ignored by Git. Keep the machine plugged in and avoid concurrent builds. The default browser port is 4185; override with HEXSCOPE_PERF_PORT. Default repeat counts: Node WASM 5, browser 3; 1–20 permitted.

```sh
pnpm perf:compare .perf/base-wasm.json .perf/candidate-wasm.json
pnpm perf:compare .perf/base-desktop.json .perf/candidate-desktop.json --json
```

The comparator uses medians, rejects mismatched environments, case IDs, input hashes, sizes and metric sets. Default relative regression signal 20%; `--threshold=...` overrides, `--check` opts into nonzero exit. A signal is not proof of a slowdown: inspect raw samples and repeat. Zero baseline and unavailable metrics are explicit. Worker counts are descriptive, not lower-is-better. Avoid gating noisy shared-runner timing until stable baselines exist.

## What is actually measured

- Node WASM: fresh process per sample, module compilation/init, first and second parse, cleaner including output materialization, linear memory capacity after each operation and after free. These are not live allocations or process RSS.
- Browser Chromium: new context for each first open, repeat open in the same context, local preview with service workers blocked. Worker request round-trip, parser-reported duration, original input byte-buffer transfer size (not total message size), UI ready time, page JS heap snapshot from CDP, observed long tasks. Batch of ten JPEGs measures queue completion and worker generation counts. The synthetic 64 MiB MOV validates compact transfer below 1 MiB; it is a sparse structural stress fixture, not a playable real-world movie.
- Phone is device emulation, not physical iPhone RAM/CPU. CDP heap excludes worker WASM and browser-native buffers. Neither report claims peak RSS. Browser contexts do not guarantee a cold OS cache. Fixture hashes are public/synthetic inputs; no arbitrary user files or findings are recorded.
- Asset transfer sizes remain `pnpm perf:size --check`. Core native latency remains `cargo bench -p hexscope-core --bench parse`.

Next: measure streamed search and navigation, large real PDF/ZIP/Office fixtures, physical-device memory and CPU profiles. Use the initial reports as evidence before binary-size refactoring. Do not compare a Node WASM run with a browser report.
